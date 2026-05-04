use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub language: String,
    pub confidence: f32,
    pub name: String,
}
