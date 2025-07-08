use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bootstrap {
    pub cameras: HashMap<String, Camera>,
    pub nvr: Nvr,
}

impl From<BootstrapRawResponse> for Bootstrap {
    fn from(value: BootstrapRawResponse) -> Self {
        Self {
            nvr: value.nvr,
            cameras: value
                .cameras
                .into_iter()
                .map(|c| (c.id.clone(), c))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub(crate) struct BootstrapRawResponse {
    pub cameras: Vec<Camera>,
    pub nvr: Nvr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Camera {
    pub id: String,
    pub name: String,
    pub mac: String,
    pub model: Option<String>,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Nvr {
    pub id: String,
    pub name: String,
    pub version: String,
    pub timezone: String,
}
