use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectEvent {
    pub id: String,
    pub camera_id: String,
    pub camera_name: String,
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
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
        let detection_type = self.format_detection_type();
        let start_date = self.start.format("%Y-%m-%d");
        let start_time = self.start.format("%H-%M-%S");
        let end_time = self
            .end
            .map(|e| e.format("%H-%M-%S").to_string())
            .unwrap_or_else(|| "ongoing".to_string());

        format_string
            .replace("{camera_name}", &self.camera_name)
            .replace("{camera_id}", &self.camera_id)
            .replace("{date}", &start_date.to_string())
            .replace("{time}", &start_time.to_string())
            .replace("{end_time}", &end_time)
            .replace("{detection_type}", &detection_type)
            .replace("{event_id}", &self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub action: WebSocketAction,
    pub new_update_id: Option<String>,
    pub model_key: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSocketAction {
    #[serde(rename = "add")]
    Add,
    #[serde(rename = "update")]
    Update,
    #[serde(rename = "remove")]
    Remove,
}
