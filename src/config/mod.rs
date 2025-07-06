use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub unifi: UnifiConfig,
    pub database: DatabaseConfig,
    pub backup: Option<BackupConfig>,
    pub notifications: Option<NotificationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiConfig {
    pub address: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub verify_ssl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub rsync_host: String,
    pub rsync_user: String,
    pub rsync_path: String,
    pub ssh_key_path: Option<PathBuf>,
    pub borg_repo: String,
    pub borg_passphrase: Option<String>,
    pub retention_days: u32,
    pub file_structure_format: String,
    pub detection_types: Vec<String>,
    pub ignore_cameras: Vec<String>,
    pub cameras: Vec<String>,
    pub max_event_length_seconds: u32,
    pub download_buffer_size: u64,
    pub parallel_uploads: u32,
    pub purge_interval_hours: u32,
    pub skip_missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub email_from: Option<String>,
    pub email_to: Option<String>,
}

pub use clap::Parser;
use tracing::info;

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

pub fn from_file_or_const<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    if let Some(s) = s.strip_prefix("file:") {
        std::fs::read_to_string(s).map_err(serde::de::Error::custom)
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
    let verify_ssl = prompt_with_default("Verify SSL (true/false)", "true")?;

    let rsync_host = prompt_with_default("Rsync host", "rsync.net")?;
    let rsync_user = prompt_with_default("Rsync user", "user")?;
    let rsync_path = prompt_with_default("Rsync path", "unifi-protect")?;
    let ssh_key_path = prompt_with_default("SSH key path (optional)", "")?;
    let borg_repo = prompt_with_default("Borg repository", "user@rsync.net:unifi-protect")?;
    let borg_passphrase = prompt_with_default("Borg passphrase (optional)", "")?;
    let retention_days = prompt_with_default("Retention days", "30")?;
    let detection_types =
        prompt_with_default("Detection types (comma-separated)", "motion,person,vehicle")?;
    let file_structure_format = prompt_with_default(
        "File structure format",
        "{camera_name}/{date}/{time}_{detection_type}.mp4",
    )?;
    let ignore_cameras = prompt_with_default("Ignore cameras (comma-separated, optional)", "")?;
    let cameras = prompt_with_default("Cameras to backup (comma-separated, optional)", "")?;
    let max_event_length_seconds = prompt_with_default("Max event length (seconds)", "300")?;
    let download_buffer_size = prompt_with_default("Download buffer size (bytes)", "8192")?;
    let parallel_uploads = prompt_with_default("Parallel uploads", "3")?;
    let purge_interval_hours = prompt_with_default("Purge interval (hours)", "24")?;
    let skip_missing = prompt_with_default("Skip missing files (true/false)", "false")?;

    let database_path = prompt_with_default(
        "Database path",
        &format!(
            "{}/unifi-protect-backup/events.db",
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

    let ssh_key_path_line = if ssh_key_path.is_empty() {
        "# ssh_key_path = \"/path/to/ssh/key\"".to_string()
    } else {
        format!("ssh_key_path = \"{ssh_key_path}\"")
    };

    let borg_passphrase_line = if borg_passphrase.is_empty() {
        "# borg_passphrase = \"your-passphrase\"".to_string()
    } else {
        format!("borg_passphrase = \"{borg_passphrase}\"")
    };

    let config = format!(
        r#"[unifi]
address = "{address}"
port = {port}
username = "{username}"
password = "{password}"
verify_ssl = {verify_ssl}

[backup]
rsync_host = "{rsync_host}"
rsync_user = "{rsync_user}"
rsync_path = "{rsync_path}"
{ssh_key_path_line}
borg_repo = "{borg_repo}"
{borg_passphrase_line}
retention_days = {retention_days}
detection_types = [{detection_types_array}]
file_structure_format = "{file_structure_format}"
ignore_cameras = [{ignore_cameras_array}]
cameras = [{cameras_array}]
max_event_length_seconds = {max_event_length_seconds}
download_buffer_size = {download_buffer_size}
parallel_uploads = {parallel_uploads}
purge_interval_hours = {purge_interval_hours}
skip_missing = {skip_missing}

[database]
path = "{database_path}"
"#
    );

    Ok(config)
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
