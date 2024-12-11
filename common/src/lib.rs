use crate::errors::*;
use chrono::NaiveDateTime;
use colored::*;
use serde::{Deserialize, Serialize};
use std::iter::FromIterator;
use std::ops::Deref;
use std::str::FromStr;

pub mod api;
pub mod auth;
pub mod config;
pub mod errors;
pub mod http;
pub mod utils;

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

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PkgArtifact {
    pub name: String,
    pub version: String,
    pub url: String,
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
        self.artifacts.push(artifact);
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
}

impl Status {
    pub fn fancy(&self) -> String {
        match self {
            Status::Good => "GOOD ".green().to_string(),
            Status::Bad => "BAD  ".red().to_string(),
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
            _ => bail!("Unknown status: {:?}", s),
        }
    }
}
