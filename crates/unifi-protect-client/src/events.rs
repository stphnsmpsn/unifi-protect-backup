use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fmt::Display};
use uuid::Uuid;

use crate::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectEvent {
    pub id: String,
    pub camera_id: String,
    pub camera_name: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub event_type: EventType,
    pub smart_detect_types: Vec<SmartDetectType>,
    pub thumbnail_id: Option<String>,
    pub heatmap_id: Option<String>,
    pub is_finished: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    Motion,
    Ring,
    Line,
    SmartDetect,
}

impl Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::Motion => write!(f, "motion"),
            EventType::Ring => write!(f, "ring"),
            EventType::Line => write!(f, "line"),
            EventType::SmartDetect => write!(f, "smartdetect"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SmartDetectType {
    Person,
    Vehicle,
    Package,
    Animal,
    Face,
    LicensePlate,
}

impl ProtectEvent {
    pub fn should_backup(&self, detection_types: &[String]) -> bool {
        if detection_types.is_empty() {
            return true;
        }

        match &self.event_type {
            EventType::Motion => detection_types.contains(&"motion".to_string()),
            EventType::Ring => detection_types.contains(&"ring".to_string()),
            EventType::Line => detection_types.contains(&"line".to_string()),
            EventType::SmartDetect => {
                for smart_type in &self.smart_detect_types {
                    let type_str = match smart_type {
                        SmartDetectType::Person => "person",
                        SmartDetectType::Vehicle => "vehicle",
                        SmartDetectType::Package => "package",
                        SmartDetectType::Animal => "animal",
                        SmartDetectType::Face => "face",
                        SmartDetectType::LicensePlate => "license_plate",
                    };

                    if detection_types.contains(&type_str.to_string()) {
                        return true;
                    }
                }
                false
            }
        }
    }

    pub fn format_detection_type(&self) -> String {
        match &self.event_type {
            EventType::Motion => "motion".to_string(),
            EventType::Ring => "ring".to_string(),
            EventType::Line => "line".to_string(),
            EventType::SmartDetect => {
                if self.smart_detect_types.is_empty() {
                    "smart_detect".to_string()
                } else {
                    let types: Vec<String> = self
                        .smart_detect_types
                        .iter()
                        .map(|t| match t {
                            SmartDetectType::Person => "person",
                            SmartDetectType::Vehicle => "vehicle",
                            SmartDetectType::Package => "package",
                            SmartDetectType::Animal => "animal",
                            SmartDetectType::Face => "face",
                            SmartDetectType::LicensePlate => "license_plate",
                        })
                        .map(|s| s.to_string())
                        .collect();
                    types.join("_")
                }
            }
        }
    }

    pub fn format_filename(&self, format_string: &str) -> String {
        let start_time = self.start_time.map_or_else(Utc::now, |t| {
            DateTime::<Utc>::from_timestamp_millis(t).unwrap_or_else(Utc::now)
        });
        let end_time = self
            .end_time
            .map(|t| DateTime::<Utc>::from_timestamp_millis(t).unwrap_or_else(Utc::now));

        let detection_type = self.format_detection_type();
        let start_date = start_time.format("%Y-%m-%d");
        let start_time = start_time.format("%H-%M-%S");
        let end_time = end_time
            .map(|e| e.format("%H-%M-%S").to_string())
            .unwrap_or_else(|| "ongoing".to_string());

        format_string
            .replace(
                "{camera_name}",
                &self
                    .camera_name
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string()),
            )
            .replace("{camera_id}", &self.camera_id)
            .replace("{date}", &start_date.to_string())
            .replace("{time}", &start_time.to_string())
            .replace("{end_time}", &end_time)
            .replace("{detection_type}", &detection_type)
            .replace("{event_id}", &self.id)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct WebSocketMessage {
    pub action_frame: WebSocketActionFrame,
    pub data_frame: WebSocketDataFrame,
}

impl WebSocketMessage {
    pub fn from_binary(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let frames = ProtectWebSocketRawFrames::try_from(data)?;

        let action_frame = serde_json::from_str::<WebSocketActionFrame>(&frames.action)?;
        let data_frame = serde_json::from_str::<WebSocketDataFrame>(&frames.data)?;

        Ok(WebSocketMessage {
            action_frame,
            data_frame,
        })
    }
}

#[derive(Debug)]
pub struct ProtectWebSocketRawFrames {
    pub action: String,
    pub data: String,
}

impl TryFrom<&[u8]> for ProtectWebSocketRawFrames {
    type Error = Error;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() < 16 {
            return Err(Error::Api("Binary data too short".into()));
        }

        // Read action frame length from first header
        let action_length = if data[6] == 0 {
            // Single byte length at position 7
            data[7] as usize
        } else {
            // Multi-byte length (big-endian u16 at positions 6-7)
            u16::from_be_bytes([data[6], data[7]]) as usize
        };

        let action_start = 8;
        let action_end = action_start + action_length;

        if action_end + 8 > data.len() {
            return Err(Error::Api(format!(
                "Action frame extends beyond data: {} + {} > {}",
                action_start,
                action_length,
                data.len()
            )));
        }

        // Extract action JSON
        let action_json = std::str::from_utf8(&data[action_start..action_end])
            .map_err(|_| Error::Api("Invalid UTF-8 in action frame".into()))?;

        // Read data frame length from second header
        let second_header_start = action_end;
        let data_length = if data[second_header_start + 6] == 0 {
            // Single byte length
            data[second_header_start + 7] as usize
        } else {
            // Multi-byte length (big-endian)
            u16::from_be_bytes([data[second_header_start + 6], data[second_header_start + 7]])
                as usize
        };

        let data_start = action_end + 8;
        let data_end = data_start + data_length;

        if data_end > data.len() {
            return Err(Error::Api(format!(
                "Data frame extends beyond data: {} + {} > {}",
                data_start,
                data_length,
                data.len()
            )));
        }

        // Extract data JSON
        let data_json = std::str::from_utf8(&data[data_start..data_end])
            .map_err(|_| Error::Api("Invalid UTF-8 in data frame".into()))?;

        Ok(Self {
            action: action_json.to_string(),
            data: data_json.to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct WebSocketActionFrame {
    pub action: WebSocketAction,
    pub new_update_id: Uuid,
    pub model_key: ModelKey,
    pub record_model: Option<String>,
    pub record_id: Option<String>,
    pub id: String,
    #[serde(flatten)]
    pub extra_fields: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all(deserialize = "camelCase"))]
pub enum ModelKey {
    Camera,
    Nvr,
    Event,
    Chime,
    Bridge,
    User,
    Group,
    Light,
    Liveview,
    Sensor,
    Viewer,
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all(deserialize = "camelCase"))]
pub enum Kind {
    Motion,
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct WebSocketDataFrame {
    #[serde(rename(deserialize = "type"))]
    pub kind: Option<Kind>,
    pub id: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
    #[serde(flatten)]
    pub extra_fields: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WebSocketAction {
    #[serde(rename = "add")]
    Add,
    #[serde(rename = "update")]
    Update,
}
