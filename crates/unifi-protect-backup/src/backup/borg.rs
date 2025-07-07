use std::{path::PathBuf, process::Stdio};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
// use tempfile::NamedTempFile;
use tokio::{io::AsyncWriteExt, process::Command};
use tracing::{debug, info};

use unifi_protect_client::events::ProtectEvent;

use crate::{
    Error, Result, backup,
    backup::{Backup, RemoteConfig},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct Config {
    pub rsync_host: String,
    pub rsync_user: String,
    pub rsync_path: String,
    pub ssh_key_path: Option<PathBuf>,
    pub borg_repo: String,
    pub borg_passphrase: Option<String>,
}

pub struct BorgBackup {
    pub config: backup::Config,
    pub remote: Config,
}

impl BorgBackup {
    pub fn new(config: backup::Config) -> Self {
        match config.remote.clone() {
            RemoteConfig::Borg(remote) => Self { config, remote },
        }
    }
}

#[async_trait]
impl Backup for BorgBackup {
    async fn init(&self) -> Result<()> {
        info!("Initializing borg repository: {}", self.remote.borg_repo);

        let mut cmd = Command::new("borg");
        cmd.arg("init")
            .arg("--encryption=repokey-blake2")
            .arg(&self.remote.borg_repo);

        if let Some(ref passphrase) = self.remote.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.remote.ssh_key_path {
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
            // Repository might already exist, which is OK
            if stderr.contains("already exists") {
                info!("Repository already exists, skipping initialization");
                return Ok(());
            }
            return Err(Error::Backup(format!(
                "Failed to initialize borg repository: {stderr}"
            )));
        }

        info!("Borg repository initialized successfully");
        Ok(())
    }

    async fn backup(&self, event: &ProtectEvent, video_data: &[u8]) -> Result<String> {
        let filename = event.format_filename(&self.config.file_structure_format);
        // let archive_name = format!(
        //     "{}::{}",
        //     self.remote.borg_repo,
        //     Utc::now().format("%Y-%m-%d_%H-%M-%S")
        // );

        info!("Backing up event {} as {}", event.id, filename);

        // Use current directory explicitly or specify a base path
        let base_path = std::env::current_dir()?;
        let file_path = base_path.join(&filename);

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = tokio::fs::File::create(&file_path).await?;
        file.write_all(video_data).await?;
        file.flush().await?;

        // Write video data to temporary file
        // let mut temp_file = NamedTempFile::new()?;
        // temp_file.write_all(video_data)?;
        // let temp_path = temp_file.path();

        // // Create archive with borg
        // let mut cmd = Command::new("borg");
        // cmd.arg("create")
        //     .arg("--verbose")
        //     .arg("--filter=AME")
        //     .arg("--list")
        //     .arg("--stats")
        //     .arg("--show-rc")
        //     .arg("--compression=lz4")
        //     .arg(&archive_name)
        //     .arg(temp_path);
        //
        // if let Some(ref passphrase) = self.remote.borg_passphrase {
        //     cmd.env("BORG_PASSPHRASE", passphrase);
        // }
        //
        // // Set SSH key if provided
        // if let Some(ref ssh_key) = self.remote.ssh_key_path {
        //     let ssh_cmd = format!("ssh -i {}", ssh_key.display());
        //     cmd.env("BORG_RSH", ssh_cmd);
        // }
        //
        // let output = cmd
        //     .stdout(Stdio::piped())
        //     .stderr(Stdio::piped())
        //     .output()
        //     .await?;
        //
        // if !output.status.success() {
        //     let stderr = String::from_utf8_lossy(&output.stderr);
        //     return Err(Error::Backup(format!("Borg backup failed: {stderr}")));
        // }
        //
        // let stdout = String::from_utf8_lossy(&output.stdout);
        // debug!("Borg backup output: {}", stdout);
        //
        // info!(
        //     "Successfully backed up event {} to {}",
        //     event.id, archive_name
        // );
        Ok(filename)
    }

    async fn prune(&self) -> Result<()> {
        info!(
            "Pruning old backups (retention: {} days)",
            self.config.retention_days
        );

        let mut cmd = Command::new("borg");
        cmd.arg("prune")
            .arg("--verbose")
            .arg("--list")
            .arg("--show-rc")
            .arg("--keep-daily")
            .arg(self.config.retention_days.to_string())
            .arg(&self.remote.borg_repo);

        if let Some(ref passphrase) = self.remote.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.remote.ssh_key_path {
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

    async fn list_archives(&self) -> Result<Vec<String>> {
        let mut cmd = Command::new("borg");
        cmd.arg("list").arg("--short").arg(&self.remote.borg_repo);

        if let Some(ref passphrase) = self.remote.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.remote.ssh_key_path {
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
            return Err(Error::Backup(format!("Borg list failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let archives: Vec<String> = stdout
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(archives)
    }

    async fn check_repository(&self) -> Result<()> {
        info!("Checking borg repository integrity");

        let mut cmd = Command::new("borg");
        cmd.arg("check")
            .arg("--verbose")
            .arg(&self.remote.borg_repo);

        if let Some(ref passphrase) = self.remote.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.remote.ssh_key_path {
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
            return Err(Error::Backup(format!("Borg check failed: {stderr}")));
        }

        info!("Repository integrity check passed");
        Ok(())
    }
}
