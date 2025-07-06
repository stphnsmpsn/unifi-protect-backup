use chrono::Utc;
use std::{io::Write, process::Stdio};
use tempfile::NamedTempFile;
use tokio::process::Command;
use tracing::{debug, info};

use crate::{Error, Result, config::BackupConfig, events::ProtectEvent};

pub struct BorgBackup {
    config: BackupConfig,
}

impl BorgBackup {
    pub fn new(config: BackupConfig) -> Self {
        Self { config }
    }

    pub async fn init_repository(&self) -> Result<()> {
        info!("Initializing borg repository: {}", self.config.borg_repo);

        let mut cmd = Command::new("borg");
        cmd.arg("init")
            .arg("--encryption=repokey-blake2")
            .arg(&self.config.borg_repo);

        if let Some(ref passphrase) = self.config.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.config.ssh_key_path {
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

    pub async fn backup_event(&self, event: &ProtectEvent, video_data: &[u8]) -> Result<String> {
        let filename = event.format_filename(&self.config.file_structure_format);
        let archive_name = format!(
            "{}::{}",
            self.config.borg_repo,
            Utc::now().format("%Y-%m-%d_%H-%M-%S")
        );

        info!("Backing up event {} as {}", event.id, filename);

        // Write video data to temporary file
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(video_data)?;
        let temp_path = temp_file.path();

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
            .arg(temp_path);

        if let Some(ref passphrase) = self.config.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.config.ssh_key_path {
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
            return Err(Error::Backup(format!("Borg backup failed: {stderr}")));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Borg backup output: {}", stdout);

        info!(
            "Successfully backed up event {} to {}",
            event.id, archive_name
        );
        Ok(filename)
    }

    pub async fn prune_old_backups(&self) -> Result<()> {
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
            .arg(&self.config.borg_repo);

        if let Some(ref passphrase) = self.config.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.config.ssh_key_path {
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

    pub async fn list_archives(&self) -> Result<Vec<String>> {
        let mut cmd = Command::new("borg");
        cmd.arg("list").arg("--short").arg(&self.config.borg_repo);

        if let Some(ref passphrase) = self.config.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.config.ssh_key_path {
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

    pub async fn check_repository(&self) -> Result<()> {
        info!("Checking borg repository integrity");

        let mut cmd = Command::new("borg");
        cmd.arg("check")
            .arg("--verbose")
            .arg(&self.config.borg_repo);

        if let Some(ref passphrase) = self.config.borg_passphrase {
            cmd.env("BORG_PASSPHRASE", passphrase);
        }

        // Set SSH key if provided
        if let Some(ref ssh_key) = self.config.ssh_key_path {
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
