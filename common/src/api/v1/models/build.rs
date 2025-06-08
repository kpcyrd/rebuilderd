use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RebuildRequest {
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub component: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RebuildReport {
    pub queue_id: i32,
    pub built_at: NaiveDateTime,
    pub build_log: Vec<u8>,
    pub status: BuildStatus,
    pub artifacts: Vec<RebuildArtifactReport>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildStatus {
    Good,
    Bad,
    Fail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RebuildArtifactReport {
    pub name: String,
    pub diffoscope: Option<Vec<u8>>,
    pub attestation: Option<Vec<u8>>,
    pub status: ArtifactStatus,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactStatus {
    Good,
    Bad,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rebuild {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architecture: String,
    pub backend: String,
    pub retries: u32,
    pub started_at: NaiveDateTime,
    pub built_at: NaiveDateTime,
    pub status: BuildStatus,
}
