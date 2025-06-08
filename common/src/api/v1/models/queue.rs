use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PopQueuedJobRequest {
    pub supported_backends: Vec<String>,
    pub architecture: String,
    pub supported_architectures: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueuedJob {
    pub id: i32,
    pub architecture: String,
    pub backend: String,
    pub url: String,
}
