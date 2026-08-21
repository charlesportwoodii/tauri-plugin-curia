use curia::Level;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordPayload {
    pub message: String,
    pub level: Level,
}
