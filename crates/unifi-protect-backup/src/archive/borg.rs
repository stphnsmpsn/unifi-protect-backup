use std::{path::PathBuf, process::Stdio};

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{debug, info, trace};

use crate::{Error, Result, archive, archive::Archive};

const SECONDS_PER_DAY: u64 = 24 * 60 * 60; // 86400

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct Config {
    pub ssh_key_path: Option<PathBuf>,
    pub borg_repo: String,
    pub borg_passphrase: Option<String>,
}

pub struct BorgBackup {
    pub backup_config: archive::Config,
    pub remote_config: Config,
}

impl BorgBackup {
    pub fn new(backup_config: archive::Config, remote_config: Config) -> Self {
        Self {
            backup_config,
            remote_config,
        }
    }
}

#[async_trait]
impl Archive for BorgBackup {
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
            .arg("--compression=lz4")
            .arg(&archive_name)
            .arg("./.data");    // todo(steve.sampson): don't hard code this path!

        if let Some(ref passphrase) = self.remote_config.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.remote_config.ssh_key_path {
            let ssh_cmd = format!("ssh -i {}", ssh_key.display());
            cmd.env("BORG_RSH", ssh_cmd);
        }

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

    async fn prune(&self) -> Result<()> {
        info!(
            "Pruning old backups (retention: {:?} days)",
            self.backup_config.retention_period
        );

        let mut cmd = Command::new("borg");
        cmd.arg("prune")
            .arg("--verbose")
            .arg("--list")
            .arg("--show-rc")
            .arg("--keep-daily")
            .arg((self.backup_config.retention_period.as_secs() / SECONDS_PER_DAY).to_string())
            .arg(&self.remote_config.borg_repo);

        if let Some(ref passphrase) = self.remote_config.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.remote_config.ssh_key_path {
            let ssh_cmd = format!("ssh -i {}", ssh_key.display());
            cmd.env("BORG_RSH", ssh_cmd);
        }

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

        info!("Successfully pruned old backups");
        Ok(())
    }
}
