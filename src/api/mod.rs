use chrono::DateTime;
use futures_util::StreamExt;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info};

use crate::{
    Error, Result,
    config::UnifiConfig,
    events::{EventType, ProtectEvent, SmartDetectType, WebSocketMessage},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Camera {
    pub id: String,
    pub name: String,
    pub mac: String,
    pub model: String,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bootstrap {
    pub cameras: HashMap<String, Camera>,
    pub nvr: Nvr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nvr {
    pub id: String,
    pub name: String,
    pub version: String,
    pub timezone: String,
}

pub struct ProtectClient {
    client: Client,
    base_url: Url,
    config: UnifiConfig,
    auth_cookie: Option<String>,
    csrf_token: Option<String>,
}

impl ProtectClient {
    pub fn new(config: UnifiConfig) -> Result<Self> {
        let client = Client::builder()
            .danger_accept_invalid_certs(!config.verify_ssl)
            .build()?;

        let base_url = Url::parse(&format!("https://{}:{}", config.address, config.port))
            .map_err(|e| Error::General(format!("Invalid URL: {e}")))?;

        Ok(ProtectClient {
            client,
            base_url,
            config,
            auth_cookie: None,
            csrf_token: None,
        })
    }

    pub async fn login(&mut self) -> Result<()> {
        let login_url = self
            .base_url
            .join("/api/auth/login")
            .map_err(|e| Error::General(format!("Invalid URL: {e}")))?;

        let login_data = serde_json::json!({
            "username": self.config.username,
            "password": self.config.password,
            "remember": false
        });

        let response = self.client.post(login_url).json(&login_data).send().await?;

        if !response.status().is_success() {
            return Err(Error::Auth(format!("Login failed: {}", response.status())));
        }

        // Extract auth cookie
        if let Some(cookie_header) = response.headers().get("set-cookie") {
            let cookie_str = cookie_header
                .to_str()
                .map_err(|_| Error::Auth("Invalid cookie header".to_string()))?;

            if let Some(auth_cookie) = extract_auth_cookie(cookie_str) {
                self.auth_cookie = Some(auth_cookie);
            }
        }

        // Extract CSRF token from response
        let response_text = response.text().await?;
        if let Ok(json) = serde_json::from_str::<Value>(&response_text) {
            if let Some(csrf) = json.get("csrfToken").and_then(|v| v.as_str()) {
                self.csrf_token = Some(csrf.to_string());
            }
        }

        info!("Successfully logged in to UniFi Protect");
        Ok(())
    }

    pub async fn get_bootstrap(&self) -> Result<Bootstrap> {
        let bootstrap_url = self
            .base_url
            .join("/proxy/protect/api/bootstrap")
            .map_err(|e| Error::General(format!("Invalid URL: {e}")))?;

        let mut request = self.client.get(bootstrap_url);

        if let Some(ref cookie) = self.auth_cookie {
            request = request.header("Cookie", cookie);
        }

        if let Some(ref csrf) = self.csrf_token {
            request = request.header("X-CSRF-Token", csrf);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(Error::Api(format!(
                "Bootstrap request failed: {}",
                response.status()
            )));
        }

        let bootstrap_data: Value = response.json().await?;

        // Parse cameras
        let mut cameras = HashMap::new();
        if let Some(camera_array) = bootstrap_data.get("cameras").and_then(|v| v.as_array()) {
            for camera_data in camera_array {
                if let Ok(camera) = parse_camera(camera_data) {
                    cameras.insert(camera.id.clone(), camera);
                }
            }
        }

        // Parse NVR info
        let nvr = if let Some(nvr_data) = bootstrap_data.get("nvr") {
            parse_nvr(nvr_data)?
        } else {
            return Err(Error::Api("No NVR data in bootstrap".to_string()));
        };

        Ok(Bootstrap { cameras, nvr })
    }

    pub async fn download_event_video(&self, event: &ProtectEvent) -> Result<Vec<u8>> {
        let download_url = self
            .base_url
            .join(&format!("/proxy/protect/api/events/{}/video", event.id))
            .map_err(|e| Error::General(format!("Invalid URL: {e}")))?;

        let mut request = self.client.get(download_url);

        if let Some(ref cookie) = self.auth_cookie {
            request = request.header("Cookie", cookie);
        }

        if let Some(ref csrf) = self.csrf_token {
            request = request.header("X-CSRF-Token", csrf);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(Error::Api(format!(
                "Video download failed: {} for event {}",
                response.status(),
                event.id
            )));
        }

        let video_data = response.bytes().await?;
        Ok(video_data.to_vec())
    }

    pub async fn connect_websocket(&self) -> Result<mpsc::Receiver<ProtectEvent>> {
        let ws_url = format!(
            "wss://{}:{}/proxy/protect/ws/updates",
            self.config.address, self.config.port
        );

        let mut request =
            tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(ws_url)
                .map_err(|e| Error::WebSocket(Box::new(e)))?;

        if let Some(ref cookie) = self.auth_cookie {
            request.headers_mut().insert(
                "Cookie",
                cookie
                    .parse()
                    .map_err(|_| Error::Api("Invalid cookie".to_string()))?,
            );
        }

        let (ws_stream, _) = connect_async(request).await?;
        let (_ws_sender, mut ws_receiver) = ws_stream.split();

        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            while let Some(message) = ws_receiver.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        debug!("WebSocket message: {}", text);

                        if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(&text) {
                            if let Ok(event) = parse_protect_event(&ws_message) {
                                if let Err(e) = tx.send(event).await {
                                    error!("Failed to send event through channel: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket connection closed");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(rx)
    }
}

fn extract_auth_cookie(cookie_str: &str) -> Option<String> {
    // Parse the Set-Cookie header to extract the auth token
    if let Some(start) = cookie_str.find("TOKEN=") {
        let start = start + 6; // Skip "TOKEN="
        if let Some(end) = cookie_str[start..].find(';') {
            return Some(format!("TOKEN={}", &cookie_str[start..start + end]));
        }
    }
    None
}

fn parse_camera(camera_data: &Value) -> Result<Camera> {
    Ok(Camera {
        id: camera_data
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        name: camera_data
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        mac: camera_data
            .get("mac")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        model: camera_data
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        is_connected: camera_data
            .get("isConnected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    })
}

fn parse_nvr(nvr_data: &Value) -> Result<Nvr> {
    Ok(Nvr {
        id: nvr_data
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        name: nvr_data
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        version: nvr_data
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        timezone: nvr_data
            .get("timezone")
            .and_then(|v| v.as_str())
            .unwrap_or("UTC")
            .to_string(),
    })
}

fn parse_protect_event(ws_message: &WebSocketMessage) -> Result<ProtectEvent> {
    let data = &ws_message.data;

    let id = data
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Event("Missing event ID".to_string()))?
        .to_string();

    let camera_id = data
        .get("camera")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Event("Missing camera ID".to_string()))?
        .to_string();

    let camera_name = data
        .get("cameraName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let start_timestamp = data
        .get("start")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Error::Event("Missing start timestamp".to_string()))?;

    let start = DateTime::from_timestamp_millis(start_timestamp)
        .ok_or_else(|| Error::Event("Invalid start timestamp".to_string()))?;

    let end = data
        .get("end")
        .and_then(|v| v.as_i64())
        .and_then(DateTime::from_timestamp_millis);

    let event_type = data
        .get("type")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "motion" => EventType::Motion,
            "ring" => EventType::Ring,
            "line" => EventType::Line,
            "smartDetectZone" => EventType::SmartDetect,
            _ => EventType::Motion,
        })
        .unwrap_or(EventType::Motion);

    let smart_detect_types = data
        .get("smartDetectTypes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .filter_map(|s| match s {
                    "person" => Some(SmartDetectType::Person),
                    "vehicle" => Some(SmartDetectType::Vehicle),
                    "package" => Some(SmartDetectType::Package),
                    "animal" => Some(SmartDetectType::Animal),
                    "face" => Some(SmartDetectType::Face),
                    "licensePlate" => Some(SmartDetectType::LicensePlate),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();

    let thumbnail_id = data
        .get("thumbnailId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let heatmap_id = data
        .get("heatmapId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let is_finished = end.is_some();

    Ok(ProtectEvent {
        id,
        camera_id,
        camera_name,
        start,
        end,
        event_type,
        smart_detect_types,
        thumbnail_id,
        heatmap_id,
        is_finished,
    })
}
