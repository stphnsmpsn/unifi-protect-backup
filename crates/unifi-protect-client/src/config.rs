use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "kebab-case"))]
pub struct UnifiConfig {
    pub address: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub verify_ssl: bool,
}
