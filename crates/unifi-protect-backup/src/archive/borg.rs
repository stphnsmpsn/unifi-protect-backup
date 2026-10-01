use std::{path::PathBuf, process::Stdio, sync::Arc};

use async_trait::async_trait;
use chrono::Utc;
use metered::{ErrorCount, HitCount, ResponseTime, Throughput};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{debug, info, trace};

use crate::{Error, Result, archive, archive::Archive, task::Prune};

const SECONDS_PER_DAY: u64 = 24 * 60 * 60; // 86400

/// How long a borg command waits for the repository or cache lock before giving up. Prune and
/// compact run right after a create in the same task, so the lock is normally free; the wait
/// covers another client (a restore, a manual compact) holding it for a while.
const LOCK_WAIT_SECS: &str = "300";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct Config {
    pub ssh_key_path: Option<PathBuf>,
    pub borg_repo: String,
    pub borg_passphrase: Option<String>,
    pub append_only: bool,
    pub source_path: PathBuf,
}

pub struct BorgBackup {
    pub backup_config: archive::Config,
    pub remote_config: Config,
    pub metrics: Arc<Metrics>,
}

impl BorgBackup {
    pub fn new(
        backup_config: archive::Config,
        remote_config: Config,
        metrics: Arc<Metrics>,
    ) -> Self {
        Self {
            backup_config,
            remote_config,
            metrics,
        }
    }
}

#[metered::metered(registry = Metrics, visibility = pub)]
impl BorgBackup {
    #[tracing::instrument(skip(self))]
    #[measure([HitCount, Throughput, ErrorCount, ResponseTime])]
    async fn archive(&self) -> Result<String> {
        let archive_name = format!(
            "{}::{}",
            self.remote_config.borg_repo,
            Utc::now().format("%Y-%m-%d_%H-%M-%S")
        );

        // Create archive with borg
        let mut cmd = Command::new("borg");
        cmd.arg("create")
            .arg("--verbose")
            .arg("--filter=AME")
            .arg("--list")
            .arg("--stats")
            .arg("--show-rc")
            .arg("--lock-wait")
            .arg(LOCK_WAIT_SECS)
            .arg("--compression=lz4")
            .arg(&archive_name)
            .arg(&self.remote_config.source_path);

        self.apply_env(&mut cmd);

        debug!("Creating Archive: {archive_name}");

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Backup(format!("Borg backup failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        trace!("Borg backup output: {}", stdout);

        info!(
            archive_name = archive_name,
            "Successfully backed up archive",
        );

        Ok(archive_name)
    }

    /// Tags every archive outside the retention window as deleted. This runs with an
    /// append-only key too: in append-only mode borg records the deletion as a new transaction
    /// (delayed deletion) and the space comes back when the repository is compacted, which for a
    /// hosted repository such as BorgBase is its server-side compaction. Skipping prune for
    /// append-only targets, as this used to, meant no archive was ever tagged and the repository
    /// could only grow.
    #[tracing::instrument(skip(self))]
    #[measure([HitCount, Throughput, ErrorCount, ResponseTime])]
    async fn prune(&self) -> Result<()> {
        let keep_daily = self.backup_config.retention_period.as_secs() / SECONDS_PER_DAY;
        info!(
            keep_daily,
            append_only = self.remote_config.append_only,
            "Pruning old archives"
        );

        let mut cmd = Command::new("borg");
        cmd.arg("prune")
            .arg("--verbose")
            .arg("--list")
            .arg("--show-rc")
            .arg("--lock-wait")
            .arg(LOCK_WAIT_SECS)
            .arg("--keep-daily")
            .arg(keep_daily.to_string())
            .arg(&self.remote_config.borg_repo);
        self.apply_env(&mut cmd);

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Backup(format!("Borg prune failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Borg prune output: {}", stdout);

        info!("Successfully pruned old archives");
        Ok(())
    }

    /// Frees the space of pruned archives. Since borg 1.2 prune only tags archives; nothing is
    /// reclaimed until `borg compact` runs. An append-only key cannot compact (the server refuses
    /// anything but appends), so for those targets this is left to the server and logged.
    #[tracing::instrument(skip(self))]
    #[measure([HitCount, Throughput, ErrorCount, ResponseTime])]
    async fn compact(&self) -> Result<()> {
        if self.remote_config.append_only {
            info!(
                "Append-only target: compaction is the server's job (enable server-side \
                 compaction on BorgBase, or run `borg compact` with a full-access key)"
            );
            return Ok(());
        }

        info!("Compacting repository");

        let mut cmd = Command::new("borg");
        cmd.arg("compact")
            .arg("--verbose")
            .arg("--show-rc")
            .arg("--lock-wait")
            .arg(LOCK_WAIT_SECS)
            .arg(&self.remote_config.borg_repo);
        self.apply_env(&mut cmd);

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Backup(format!("Borg compact failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Borg compact output: {}", stdout);

        info!("Successfully compacted repository");
        Ok(())
    }
}

impl BorgBackup {
    /// The passphrase and ssh key every borg command needs.
    fn apply_env(&self, cmd: &mut Command) {
        if let Some(ref passphrase) = self.remote_config.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }
        if let Some(ref ssh_key) = self.remote_config.ssh_key_path {
            let ssh_cmd = format!("ssh -i {}", ssh_key.display());
            cmd.env("BORG_RSH", ssh_cmd);
        }
    }
}

#[async_trait]
impl Archive for BorgBackup {
    async fn archive(&self) -> Result<String> {
        self.archive().await
    }
}

#[async_trait]
impl Prune for BorgBackup {
    /// Prune, then reclaim: the two steps borg needs to actually shrink a repository.
    async fn prune(&self) -> Result<()> {
        self.prune().await?;
        self.compact().await
    }
}
