use futures_util::StreamExt;
use reqwest::{Client, RequestBuilder, Url};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    Connector, connect_async, connect_async_tls_with_config, tungstenite::Message,
};
use tracing::{error, info, warn};

use crate::{
    config::UnifiConfig,
    error::{Error, Result},
    events::WebSocketMessage,
    models::{Bootstrap, BootstrapRawResponse},
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

    fn add_headers(&self, mut builder: RequestBuilder) -> RequestBuilder {
        if let Some(ref cookie) = self.auth_cookie {
            builder = builder.header("Cookie", cookie);
        }

        if let Some(ref csrf) = self.csrf_token {
            builder = builder.header("X-CSRF-Token", csrf);
        }

        builder
    }

    pub async fn get_bootstrap(&self) -> Result<Bootstrap> {
        let bootstrap_url = self
            .base_url
            .join("/proxy/protect/api/bootstrap")
            .map_err(|e| Error::General(format!("Invalid URL: {e}")))?;

        let request = self.client.get(bootstrap_url);
        let request = self.add_headers(request);

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(Error::Api(format!(
                "Bootstrap request failed: {}",
                response.status()
            )));
        }

        let bootstrap_value: Value = response.json().await?;
        let bootstrap_raw_response: BootstrapRawResponse = serde_json::from_value(bootstrap_value)?;
        let bootstrap = Bootstrap::from(bootstrap_raw_response);

        Ok(bootstrap)
    }

    pub async fn download_event_video(
        &self,
        camera_id: &str,
        start: i64,
        end: i64,
    ) -> Result<Vec<u8>> {
        let download_url = self
            .base_url
            .join(&format!(
                "/proxy/protect/api/video/export?camera={camera_id}&start={start}&end={end}",
            ))
            .map_err(|e| Error::General(format!("Invalid URL: {e}")))?;

        let request = self.client.get(download_url);
        let request = self.add_headers(request);

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

    pub async fn connect_websocket(&self) -> Result<mpsc::Receiver<WebSocketMessage>> {
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
            while let Some(message) = ws_receiver.next().await {
                match message {
                    Ok(Message::Binary(binary)) => {
                        let Ok(ws_message) = WebSocketMessage::from_binary(&binary)
                            .inspect_err(|e| warn!(error = ?e, "Error parsing message"))
                        else {
                            continue;
                        };

                        if let Err(e) = tx.send(ws_message).await {
                            error!("Failed to send event through channel: {}", e);
                            break;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_bootstrap_data() {
        let data = r#"{
            "cameras": [
                {
                  "id": "1",
                  "name": "Test Camera",
                  "mac": "",
                  "model": "",
                  "isConnected": true
                }
            ],
            "nvr": {
               "id": "",
               "name": "",
               "version": "",
               "timezone": "UTC"
            }
        }"#;

        let bootstrap_raw = serde_json::from_str::<BootstrapRawResponse>(data);
        assert!(bootstrap_raw.is_ok());
        let _ = Bootstrap::from(bootstrap_raw.expect("infallible"));
    }
}
