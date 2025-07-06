use thiserror::Error;

pub mod api;
pub mod backup;
pub mod config;
pub mod database;

pub mod events;
pub use config::Config;


pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),

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

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        Error::WebSocket(Box::new(error))
    }
}
