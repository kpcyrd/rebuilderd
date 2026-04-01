use rebuilderd_common::errors::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Deserialize, PartialEq)]
pub struct SyncConfigFile {
    #[serde(rename = "profile")]
    pub profiles: HashMap<String, SyncProfile>,
}

impl SyncConfigFile {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<SyncConfigFile> {
        let buf = fs::read_to_string(path).context("Failed to read config file")?;
        let config = toml::from_str(&buf).context("Failed to load config")?;
        Ok(config)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct SyncProfile {
    pub distro: String,

    pub sync_method: Option<String>,

    #[deprecated]
    pub suite: Option<String>,

    #[serde(default)]
    pub components: Vec<String>,

    #[serde(default)]
    pub releases: Vec<SyncRelease>,

    #[deprecated]
    pub architecture: Option<String>,

    #[serde(default)]
    pub architectures: Vec<String>,

    pub source: String,

    #[serde(default)]
    pub maintainers: Vec<String>,

    #[serde(default)]
    pub pkgs: Vec<String>,

    #[serde(default)]
    pub excludes: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum SyncRelease {
    Release(String),
    ReleaseWithSource {
        name: String,
        source: Option<String>,
    },
}

impl SyncRelease {
    pub fn new<I: Into<String>>(release: I) -> SyncRelease {
        SyncRelease::Release(release.into())
    }

    pub fn source<'a>(&'a self, default_source: &'a str) -> &'a str {
        match self {
            SyncRelease::Release(_release) => default_source,
            SyncRelease::ReleaseWithSource { source, .. } => {
                source.as_deref().unwrap_or(default_source)
            }
        }
    }

    pub fn name(&self) -> &str {
        match self {
            SyncRelease::Release(release) => release,
            SyncRelease::ReleaseWithSource { name, .. } => name,
        }
    }
}

impl FromStr for SyncRelease {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SyncRelease::Release(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sync_profile() {
        let config = r#"
[profile."debian-telegram"]
distro = "debian"
components = ["main"]
architectures = ["amd64"]
releases = ["trixie-backports"]
pkgs = ["telegram-desktop*"]
source = "http://deb.debian.org/debian"
"#;
        let config = toml::from_str::<SyncConfigFile>(config).unwrap();
        assert_eq!(
            config,
            SyncConfigFile {
                profiles: [(
                    "debian-telegram".to_string(),
                    SyncProfile {
                        distro: "debian".to_string(),
                        sync_method: None,
                        suite: None,
                        components: vec!["main".to_string()],
                        releases: vec![SyncRelease::new("trixie-backports")],
                        architecture: None,
                        architectures: vec!["amd64".to_string()],
                        source: "http://deb.debian.org/debian".to_string(),
                        maintainers: vec![],
                        pkgs: vec!["telegram-desktop*".to_string()],
                        excludes: vec![],
                    }
                )]
                .into_iter()
                .collect()
            }
        );
    }

    #[test]
    fn test_parse_sync_profile_multiple_sources() {
        let config = r#"
[profile."debian-telegram"]
distro = "debian"
components = ["main"]
architectures = ["amd64"]
releases = ["trixie-backports", {name = "trixie-backports-debug", source = "http://deb.debian.org/debian-debug"}]
pkgs = ["telegram-desktop*"]
source = "http://deb.debian.org/debian"
"#;
        let config = toml::from_str::<SyncConfigFile>(config).unwrap();
        assert_eq!(
            config,
            SyncConfigFile {
                profiles: [(
                    "debian-telegram".to_string(),
                    SyncProfile {
                        distro: "debian".to_string(),
                        sync_method: None,
                        suite: None,
                        components: vec!["main".to_string()],
                        releases: vec![
                            SyncRelease::new("trixie-backports"),
                            SyncRelease::ReleaseWithSource {
                                name: "trixie-backports-debug".to_string(),
                                source: Some("http://deb.debian.org/debian-debug".to_string()),
                            }
                        ],
                        architecture: None,
                        architectures: vec!["amd64".to_string()],
                        source: "http://deb.debian.org/debian".to_string(),
                        maintainers: vec![],
                        pkgs: vec!["telegram-desktop*".to_string()],
                        excludes: vec![],
                    }
                )]
                .into_iter()
                .collect()
            }
        );
    }
}
