use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub unifi: Option<UnifiConfig>,
    pub backup: Option<BackupConfig>,
    pub database: Option<DatabaseConfig>,
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

#[derive(Parser, Debug)]
pub struct Args<T: serde::de::DeserializeOwned + Clone + Send + Sync + 'static> {
    #[arg(short, long, env, value_parser = toml_from_file::<T>)]
    pub config: T,
    #[arg(short, long, env, default_value = "false")]
    pub validate: bool,
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
