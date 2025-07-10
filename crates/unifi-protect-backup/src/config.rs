use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use clap::Parser;
use serde::{Deserialize, Serialize};
use tracing::info;
use unifi_protect_client::config::UnifiConfig;

use crate::{Result, archive, backup};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct Config {
    pub unifi: UnifiConfig,
    pub database: DatabaseConfig,
    pub backup: backup::Config,
    pub archive: archive::Config,
    pub notifications: Option<NotificationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct DatabaseConfig {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct NotificationConfig {
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub email_from: Option<String>,
    pub email_to: Option<String>,
}

#[derive(Parser, Debug)]
pub struct Args<T: serde::de::DeserializeOwned + Clone + Send + Sync + 'static> {
    #[arg(short, long, env, value_parser = toml_from_file::<T>)]
    pub config: Option<T>,
    #[arg(short, long, env, default_value = "false")]
    pub validate: bool,
}

impl<T: serde::de::DeserializeOwned + Clone + Send + Sync + 'static> Args<T> {
    pub fn get_config(&self) -> Result<T> {
        if let Some(config) = &self.config {
            Ok(config.clone())
        } else {
            let default_path = default_config_path();
            toml_from_file(&default_path)
        }
    }
}

pub fn default_config_path() -> String {
    if let Ok(home_dir) = std::env::var("HOME") {
        format!("{home_dir}/.unifi-protect-backup/config.toml")
    } else {
        "config.toml".to_string()
    }
}

pub fn toml_from_file<T: serde::de::DeserializeOwned>(path: &str) -> Result<T> {
    let toml = std::fs::read_to_string(path)?;
    let config_json = toml::from_str(&toml)?;
    let config = serde_json::from_value(config_json)?;
    Ok(config)
}

pub fn from_file_const_or_env<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    if let Some(s) = s.strip_prefix("file:") {
        std::fs::read_to_string(s).map_err(serde::de::Error::custom)
    } else if let Some(s) = s.strip_prefix("env:") {
        std::env::var(s).map_err(|e| {
            serde::de::Error::custom(format!("Environment variable '{s}' not found: {e}"))
        })
    } else {
        Ok(s)
    }
}

pub async fn check_and_create_config() -> Result<()> {
    let home_dir = std::env::var("HOME").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "HOME environment variable not set",
        )
    })?;

    let config_dir = Path::new(&home_dir).join(".unifi-protect-backup");
    let config_path = config_dir.join("config.toml");

    if !config_path.exists() {
        info!("Configuration file not found. Setting up initial configuration...");

        fs::create_dir_all(&config_dir).map_err(|e| {
            std::io::Error::new(e.kind(), format!("Failed to create config directory: {e}"))
        })?;

        let config_content = prompt_for_config().await?;
        fs::write(&config_path, config_content).map_err(|e| {
            std::io::Error::new(e.kind(), format!("Failed to write config file: {e}"))
        })?;

        info!("Configuration file created at: {}", config_path.display());
    }

    Ok(())
}

async fn prompt_for_config() -> Result<String> {
    println!("Welcome to UniFi Protect Backup setup!");
    println!("Press Enter to use default values shown in brackets.\n");

    let address = prompt_with_default("UniFi Protect address", "192.168.1.100")?;
    let port = prompt_with_default("Port", "443")?;
    let username = prompt_with_default("Username", "backup-user")?;
    let password = prompt_with_default("Password", "your-password")?;
    let verify_ssl = prompt_with_default("Verify SSL (true/false)", "false")?;

    // Prompt for backup target selection
    println!("\nSelect backup targets (multiple selections supported):");
    println!("1. Local filesystem");
    println!("2. Rclone (cloud storage)");
    let backup_targets = prompt_with_default("Backup targets (comma-separated)", "1")?;

    let retention_period = prompt_with_default("Backup retention period (e.g., 30d, 1w)", "30d")?;
    let poll_interval = prompt_with_default("Poll interval (e.g., 30s, 1m)", "30s")?;
    let detection_types =
        prompt_with_default("Detection types (comma-separated)", "motion,person,vehicle")?;
    let file_structure_format = prompt_with_default(
        "File structure format",
        "{camera_name}/{date}/{time}_{detection_type}.mp4",
    )?;
    let ignore_cameras = prompt_with_default("Ignore cameras (comma-separated, optional)", "")?;
    let cameras = prompt_with_default("Cameras to backup (comma-separated, optional)", "")?;
    let max_event_length = prompt_with_default("Max event length (e.g., 5m, 300s)", "5m")?;
    let download_buffer_size = prompt_with_default("Download buffer size (bytes)", "8192")?;
    let parallel_uploads = prompt_with_default("Parallel uploads", "3")?;
    let purge_interval = prompt_with_default("Purge interval (e.g., 24h, 1d)", "24h")?;
    let skip_missing = prompt_with_default("Skip missing files (true/false)", "false")?;

    // Archive configuration
    println!("\nConfiguring archive settings (for long-term storage):");
    let archive_interval = prompt_with_default("Archive interval (e.g., 1h, 1d, 1w)", "1d")?;
    let archive_retention_period =
        prompt_with_default("Archive retention period (e.g., 30d, 1y)", "365d")?;
    let archive_purge_interval =
        prompt_with_default("Archive purge interval (e.g., 1h, 1d, 1w)", "1w")?;

    // Prompt for archive target selection
    println!("\nSelect archive targets (multiple selections supported):");
    println!("1. Borg (recommended for encrypted, deduplicated storage)");
    println!("Future versions will support additional archive targets.");
    let archive_targets = prompt_with_default("Archive targets (comma-separated)", "1")?;

    let database_path = prompt_with_default(
        "Database path",
        &format!(
            "{}/.unifi-protect-backup/events.db",
            std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
        ),
    )?;

    let detection_types_array = detection_types
        .split(',')
        .map(|s| format!("\"{}\"", s.trim()))
        .collect::<Vec<_>>()
        .join(", ");

    let ignore_cameras_array = if ignore_cameras.is_empty() {
        "".to_string()
    } else {
        ignore_cameras
            .split(',')
            .map(|s| format!("\"{}\"", s.trim()))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let cameras_array = if cameras.is_empty() {
        "".to_string()
    } else {
        cameras
            .split(',')
            .map(|s| format!("\"{}\"", s.trim()))
            .collect::<Vec<_>>()
            .join(", ")
    };

    // Generate backup remote configurations based on selections
    let mut backup_remotes = Vec::new();
    for target in backup_targets.split(',') {
        match target.trim() {
            "1" | "local" => {
                let local_path = prompt_with_default("Local backup path", "./data")?;
                backup_remotes.push(format!(
                    "[[backup.remote]]\nlocal = {{ path-buf = \"{local_path}\" }}"
                ));
            }
            "2" | "rclone" => {
                println!(
                    "\nConfiguring Rclone backup #{} (cloud storage):",
                    backup_remotes.len() + 1
                );
                let (remote, base_path, stream_upload) = prompt_for_rclone_config()?;
                backup_remotes.push(format!(
                    "[[backup.remote]]\nrclone = {{ remote = \"{remote}\", base-path = \"{base_path}\", stream-upload = {stream_upload} }}"
                ));
            }
            _ => {
                let local_path = prompt_with_default("Local backup path", "./data")?;
                backup_remotes.push(format!(
                    "[[backup.remote]]\nlocal = {{ path-buf = \"{local_path}\" }}"
                ));
            }
        }
    }
    let backup_remotes_str = backup_remotes.join("\n\n");

    // Generate archive remote configurations based on selections
    let mut archive_remotes = Vec::new();
    for target in archive_targets.split(',') {
        match target.trim() {
            "1" | "borg" => {
                println!(
                    "\nConfiguring Borg archive #{} (recommended for long-term storage):",
                    archive_remotes.len() + 1
                );
                let (ssh_key_path, borg_repo, borg_passphrase) = prompt_for_borg_config()?;

                let ssh_key_path_line = if ssh_key_path.is_empty() {
                    "".to_string()
                } else {
                    format!(", ssh-key-path = \"{ssh_key_path}\"")
                };

                let borg_passphrase_line = if borg_passphrase.is_empty() {
                    "".to_string()
                } else {
                    format!(", borg-passphrase = \"{borg_passphrase}\"")
                };

                archive_remotes.push(format!("[[archive.remote]]\nborg = {{ borg-repo = \"{borg_repo}\"{ssh_key_path_line}{borg_passphrase_line} }}"));
            }
            _ => {
                println!(
                    "\nConfiguring Borg archive #{} (recommended for long-term storage):",
                    archive_remotes.len() + 1
                );
                let (ssh_key_path, borg_repo, borg_passphrase) = prompt_for_borg_config()?;

                let ssh_key_path_line = if ssh_key_path.is_empty() {
                    "".to_string()
                } else {
                    format!(", ssh-key-path = \"{ssh_key_path}\"")
                };

                let borg_passphrase_line = if borg_passphrase.is_empty() {
                    "".to_string()
                } else {
                    format!(", borg-passphrase = \"{borg_passphrase}\"")
                };

                archive_remotes.push(format!("[[archive.remote]]\nborg = {{ borg-repo = \"{borg_repo}\"{ssh_key_path_line}{borg_passphrase_line} }}"));
            }
        }
    }
    let archive_remotes_str = archive_remotes.join("\n\n");

    let config = format!(
        r#"[unifi]
address = "{address}"
port = {port}
username = "{username}"
password = "{password}"
verify-ssl = {verify_ssl}

[backup]
retention-period = "{retention_period}"
poll-interval = "{poll_interval}"
max-event-length = "{max_event_length}"
purge-interval = "{purge_interval}"
file-structure-format = "{file_structure_format}"
detection-types = [{detection_types_array}]
ignore-cameras = [{ignore_cameras_array}]
cameras = [{cameras_array}]
download-buffer-size = {download_buffer_size}
parallel-uploads = {parallel_uploads}
skip-missing = {skip_missing}

{backup_remotes_str}

[archive]
archive-interval = "{archive_interval}"
retention-period = "{archive_retention_period}"
purge-interval = "{archive_purge_interval}"

{archive_remotes_str}

[database]
path = "{database_path}"
"#
    );

    Ok(config)
}

fn prompt_for_borg_config() -> Result<(String, String, String)> {
    println!("\nConfiguring Borg backup...");

    let ssh_key_path = prompt_with_default("SSH key path (optional)", "")?;
    let borg_repo = prompt_with_default("Borg repository", "user@rsync.net:unifi-protect")?;
    let borg_passphrase = prompt_with_default("Borg passphrase (optional)", "")?;

    Ok((ssh_key_path, borg_repo, borg_passphrase))
}

fn prompt_for_rclone_config() -> Result<(String, String, bool)> {
    let remote = prompt_with_default("Rclone remote name", "s3")?;
    let base_path = prompt_with_default("Base path in remote", "unifi-protect")?;
    let stream_upload_str = prompt_with_default("Enable streaming upload (true/false)", "true")?;

    let stream_upload = stream_upload_str.to_lowercase() == "true";

    Ok((remote, base_path, stream_upload))
}

fn prompt_with_default(prompt: &str, default: &str) -> Result<String> {
    print!("{prompt} [{default}]: ");
    io::stdout()
        .flush()
        .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to flush stdout: {e}")))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| std::io::Error::new(e.kind(), format!("Failed to read input: {e}")))?;

    let input = input.trim();
    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input.to_string())
    }
}
