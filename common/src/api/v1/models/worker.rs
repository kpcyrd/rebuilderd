use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterWorkerRequest {
    pub name: String,
    pub key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Worker {
    pub id: i32,
    pub name: String,
    pub address: String,
    pub status: Option<String>,
    pub last_ping: NaiveDateTime,
    pub is_online: bool,
}
