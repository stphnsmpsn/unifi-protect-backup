use thiserror::Error;

pub mod backup;
pub mod config;
pub mod database;

pub mod context;
pub mod convert;
pub mod tasks;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ProtectClient(#[from] unifi_protect_client::error::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),

    #[error(transparent)]
    NativeTls(#[from] native_tls::Error),

    #[error("Backup process failed: {0}")]
    Backup(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("Event processing error: {0}")]
    Event(String),

    #[error("General error: {0}")]
    General(String),
}
