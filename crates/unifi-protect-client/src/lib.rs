use std::collections::HashMap;

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use reqwest::{Client, Url};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    Connector, connect_async, connect_async_tls_with_config, tungstenite::Message,
};
use tracing::{error, info, warn};

use crate::{
    config::UnifiConfig,
    error::{Error, Result},
    events::{ProtectEvent, WebSocketMessage},
    models::{Bootstrap, parse_camera, parse_nvr, parse_protect_event},
};

pub mod config;
pub mod error;
pub mod events;
pub mod models;

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

    pub async fn download_event_video(
        &self,
        camera_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<u8>> {
        let download_url = self
            .base_url
            .join(&format!(
                "/proxy/protect/api/video/export?camera={}&start={}&end={}",
                camera_id,
                start.timestamp_millis(),
                end.timestamp_millis()
            ))
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
                "Video download failed: {} for camera {}",
                response.status(),
                camera_id
            )));
        }

        let video_data = response.bytes().await?;
        Ok(video_data.to_vec())
    }

    pub async fn connect_websocket(
        &self,
        bootstrap: Bootstrap,
    ) -> Result<mpsc::Receiver<ProtectEvent>> {
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

        let (ws_stream, _) = match self.config.verify_ssl {
            true => connect_async(request).await?,
            false => {
                // Create TLS connector that accepts invalid certificates
                let tls_connector = native_tls::TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .build()?;

                let connector = Connector::NativeTls(tls_connector);
                connect_async_tls_with_config(request, None, false, Some(connector)).await?
            }
        };

        let (_ws_sender, mut ws_receiver) = ws_stream.split();

        let (tx, rx) = mpsc::channel(100);

        // todo: handle this properly; we don't want to spawn this in the background, losing
        //  the join handle and assuming we'll never get errors
        tokio::spawn(async move {
            let mut pending_motion_events: HashMap<String, WebSocketMessage> = HashMap::new();
            while let Some(message) = ws_receiver.next().await {
                match message {
                    Ok(Message::Binary(binary)) => {
                        let Ok(ws_message) = WebSocketMessage::from_binary(&binary)
                            .inspect_err(|e| warn!(error = ?e, "Error parsing message"))
                        else {
                            continue;
                        };

                        // if it's a pending motion event we insert it in our map to join the diff
                        // with later for the whole picture
                        if let Some(id) = ws_message.new_motion_event() {
                            pending_motion_events.insert(id.clone(), ws_message.clone());
                            continue;
                        }

                        let Some(id) = ws_message.backup_candidate() else {
                            // it wasn't a new motion event and also isn't a backup candidate...
                            // move along
                            continue;
                        };

                        // it is a backup candidate!
                        let Some(original) = pending_motion_events.remove(&id) else {
                            warn!(
                                "We missed the start of this motion event and can't get the start time for it to export"
                            );
                            continue;
                        };

                        let original_ws_message = original;
                        let motion_event_completed_ws_message = ws_message;
                        let known_camera = motion_event_completed_ws_message
                            .action_frame
                            .record_id
                            .as_ref()
                            .and_then(|c| bootstrap.cameras.get(c));

                        if let Ok(event) = parse_protect_event(
                            &original_ws_message,
                            &motion_event_completed_ws_message,
                            known_camera,
                        ) {
                            if let Err(e) = tx.send(event).await {
                                error!("Failed to send event through channel: {}", e);
                                break;
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
