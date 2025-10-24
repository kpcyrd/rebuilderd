use crate::errors::*;
use anyhow::bail;
use chrono::NaiveDateTime;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Deref;
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize)]
pub enum Success {
    Ok,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Worker {
    pub key: String,
    pub addr: String,
    pub status: Option<String>,
    pub last_ping: NaiveDateTime,
    pub online: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkQuery {
    pub supported_backends: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobAssignment {
    Nothing,
    Rebuild(Box<QueueItem>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuiteImport {
    pub distro: String,
    pub suite: String,
    pub groups: Vec<PkgGroup>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListPkgs {
    pub name: Option<String>,
    pub status: Option<Status>,
    pub distro: Option<String>,
    pub suite: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueList {
    pub now: NaiveDateTime,
    pub queue: Vec<QueueItem>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: i32,
    pub pkgbase: PkgGroup,
    pub version: String,
    pub queued_at: NaiveDateTime,
    pub worker_id: Option<i32>,
    pub started_at: Option<NaiveDateTime>,
    pub last_ping: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListQueue {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushQueue {
    pub name: String,
    pub version: Option<String>,
    pub priority: i32,
    pub distro: String,
    pub suite: String,
    pub architecture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DropQueueItem {
    pub name: String,
    pub version: Option<String>,
    pub distro: String,
    pub suite: String,
    pub architecture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequeueQuery {
    pub name: Option<String>,
    pub status: Option<Status>,
    pub priority: i32,
    pub distro: Option<String>,
    pub suite: Option<String>,
    pub architecture: Option<String>,
    pub reset: bool,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildStatus {
    Good,
    Bad,
    Fail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rebuild {
    pub status: BuildStatus,
    pub diffoscope: Option<String>,
    pub attestation: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PkgRelease {
    pub name: String,
    pub version: String,
    pub status: Status,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub artifact_url: String,
    pub build_id: Option<i32>,
    pub built_at: Option<NaiveDateTime>,
    pub has_diffoscope: bool,
    pub has_attestation: bool,
}

impl PkgRelease {
    pub fn new(
        name: String,
        version: String,
        distro: String,
        suite: String,
        architecture: String,
        artifact_url: String,
    ) -> PkgRelease {
        PkgRelease {
            name,
            version,
            status: Status::Unknown,
            distro,
            suite,
            architecture,
            artifact_url,
            build_id: None,
            built_at: None,
            has_diffoscope: false,
            has_attestation: false,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PkgGroup {
    pub name: String,
    pub version: String,

    pub distro: String,
    pub suite: String,
    pub architecture: String,

    pub input_url: Option<String>,
    pub artifacts: Vec<PkgArtifact>,
}
impl PkgGroup {
    pub fn new(
        name: String,
        version: String,
        distro: String,
        suite: String,
        architecture: String,
        input_url: Option<String>,
    ) -> PkgGroup {
        PkgGroup {
            name,
            version,
            distro,
            suite,
            architecture,
            input_url,
            artifacts: Vec::new(),
        }
    }

    pub fn add_artifact(&mut self, artifact: PkgArtifact) {
        // this list is always fairly short, so using contains should be fine
        if !self.artifacts.contains(&artifact) {
            self.artifacts.push(artifact);
        }
    }

    pub fn input_url(&self) -> Result<&str> {
        if let Some(input_url) = &self.input_url {
            Ok(input_url.as_str())
        } else if !self.artifacts.is_empty() {
            let mut artifacts = Vec::from_iter(self.artifacts.iter().collect::<Vec<_>>());
            artifacts.sort_by_key(|a| &a.name);
            // we've checked that artifacts is not empty
            let input = artifacts.into_iter().next().unwrap();
            Ok(&input.url)
        } else {
            bail!("Package group has no artifacts")
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PkgArtifact {
    pub name: String,
    pub version: String,
    pub url: String,
}

impl Rebuild {
    pub fn new(status: BuildStatus) -> Rebuild {
        Rebuild {
            status,
            diffoscope: None,
            attestation: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildReport {
    pub queue: QueueItem,
    pub build_log: String,
    pub rebuilds: Vec<(PkgArtifact, Rebuild)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardResponse {
    pub suites: HashMap<String, SuiteStats>,
    pub active_builds: Vec<QueueItem>,
    pub queue_length: usize,
    pub now: NaiveDateTime,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SuiteStats {
    pub good: usize,
    pub unknown: usize,
    pub bad: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PingRequest {
    pub queue_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
pub enum Status {
    #[serde(rename = "GOOD")]
    #[clap(name = "GOOD")]
    Good,
    #[serde(rename = "BAD")]
    #[clap(name = "BAD")]
    Bad,
    #[serde(rename = "UNKWN")]
    #[clap(name = "UNKWN")]
    Unknown,
    #[serde(rename = "FAIL")]
    #[clap(name = "FAIL")]
    Fail,
}

impl Status {
    pub fn fancy(&self) -> String {
        match self {
            Status::Good => "GOOD ".green().to_string(),
            Status::Bad => "BAD  ".red().to_string(),
            Status::Fail => "FAIL ".red().to_string(),
            Status::Unknown => "UNKWN".yellow().to_string(),
        }
    }
}

impl Deref for Status {
    type Target = str;

    fn deref(&self) -> &'static str {
        match self {
            Status::Good => "GOOD",
            Status::Bad => "BAD",
            Status::Fail => "FAIL",
            Status::Unknown => "UNKWN",
        }
    }
}

impl FromStr for Status {
    type Err = Error;

    fn from_str(s: &str) -> Result<Status> {
        match s {
            "GOOD" => Ok(Status::Good),
            "BAD" => Ok(Status::Bad),
            "UNKWN" => Ok(Status::Unknown),
            "FAIL" => Ok(Status::Fail),
            _ => bail!("Unknown status: {:?}", s),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PublicKeys {
    pub current: Vec<String>,
}
