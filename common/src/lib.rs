use crate::errors::*;
use colored::*;
use chrono::NaiveDateTime;
use strum_macros::{EnumString, AsRefStr, Display};
use serde::{Serialize, Deserialize};
use std::iter::FromIterator;
use std::ops::Deref;
use std::str::FromStr;

pub mod api;
pub mod auth;
pub mod config;
pub mod errors;
pub mod utils;

#[derive(Debug, Clone, Copy, PartialEq, Display, EnumString, AsRefStr, Serialize, Deserialize)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum Distro {
    Debian,
    Archlinux,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PkgRelease {
    pub name: String,
    pub version: String,
    pub status: Status,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub url: String,
    pub build_id: Option<i32>,
    pub built_at: Option<NaiveDateTime>,
    pub has_diffoscope: bool,
    pub has_attestation: bool,
    pub next_retry: Option<NaiveDateTime>,
}

impl PkgRelease {
    pub fn new(name: String, version: String, distro: Distro, suite: String, architecture: String, url: String) -> PkgRelease {
        PkgRelease {
            name,
            version,
            status: Status::Unknown,
            distro: distro.to_string(),
            suite,
            architecture,
            url,
            build_id: None,
            built_at: None,
            has_diffoscope: false,
            has_attestation: false,
            next_retry: None,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PkgGroup {
    pub base: String,
    pub version: String,

    pub distro: String,
    pub suite: String,
    pub architecture: String,

    pub input: Option<String>,
    pub artifacts: Vec<PkgArtifact>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PkgArtifact {
    pub name: String,
    pub url: String,
}

impl PkgGroup {
    pub fn new(base: String, version: String, distro: Distro, suite: String, architecture: String, input: Option<String>) -> PkgGroup {
        PkgGroup {
            base,
            version,
            distro: distro.to_string(),
            suite,
            architecture,
            input,
            artifacts: Vec::new(),
        }
    }

    pub fn add_artifact(&mut self, artifact: PkgArtifact) {
        self.artifacts.push(artifact);
    }

    pub fn input(&self) -> Result<&str> {
        if let Some(input) = &self.input {
            Ok(input.as_str())
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Status {
    #[serde(rename = "GOOD")]
    Good,
    #[serde(rename = "BAD")]
    Bad,
    #[serde(rename = "UNKWN")]
    Unknown,
}

impl Status {
    pub fn fancy(&self) -> String {
        match self {
            Status::Good    => "GOOD ".green().to_string(),
            Status::Bad     => "BAD  ".red().to_string(),
            Status::Unknown => "UNKWN".yellow().to_string(),
        }
    }
}

impl Deref for Status {
    type Target = str;

    fn deref(&self) -> &'static str {
        match self {
            Status::Good    => "GOOD",
            Status::Bad     => "BAD",
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
