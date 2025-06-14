use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamResponse {
    pub stream_id: String,
}

// for client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleUpdate {
    pub thread_id: String,
    pub title: String,
    pub status: String, // "generating", "completed", "error"
}
