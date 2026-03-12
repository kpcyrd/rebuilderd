use fancy_regex::Regex;
use rebuilderd_common::errors::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Config file types (deserialized from TOML)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StatsConfigFile {
    #[serde(default, rename = "backend")]
    pub backends: HashMap<String, BackendStats>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BackendStats {
    #[serde(default)]
    pub categories: Vec<ErrorCategory>,
}

/// A single named error category with exactly one matcher field set.
/// Categories are evaluated in order; the first match wins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorCategory {
    pub name: String,

    // --- log matchers ---
    pub log_has: Option<String>,
    pub log_has_re: Option<String>,
    pub log_has_any: Option<Vec<String>>,
    pub log_has_all: Option<Vec<String>>,

    // --- diffoscope matchers ---
    pub diff_has: Option<String>,
    pub diff_has_re: Option<String>,
    pub diff_has_any: Option<Vec<String>>,

    /// Matches when a non-empty diffoscope log is present.
    #[serde(default)]
    pub has_diff: bool,

    /// Always matches — use as the last catch-all category.
    #[serde(default)]
    pub catch_all: bool,

    /// Skip this category for these architectures (e.g. ["all"]).
    #[serde(default)]
    pub exclude_architectures: Vec<String>,
}

impl ErrorCategory {
    /// Returns true if this category matches the given log and diffoscope.
    pub fn matches(
        &self,
        log: &str,
        diff: &str,
        architecture: Option<&str>,
    ) -> Result<bool> {
        if let Some(arch) = architecture {
            if self.exclude_architectures.iter().any(|x| arch.starts_with(x.as_str())) {
                return Ok(false);
            }
        }

        if let Some(s) = &self.log_has {
            return Ok(log.contains(s.as_str()));
        }
        if let Some(re) = &self.log_has_re {
            // (?s) enables DOTALL so '.' matches newlines, mirroring Python's re.DOTALL
            let re = Regex::new(&format!("(?s){re}"))
                .with_context(|| format!("Invalid regex in category {:?}: {re}", self.name))?;
            return Ok(re.is_match(log)?);
        }
        if let Some(strs) = &self.log_has_any {
            return Ok(strs.iter().any(|s| log.contains(s.as_str())));
        }
        if let Some(strs) = &self.log_has_all {
            return Ok(strs.iter().all(|s| log.contains(s.as_str())));
        }
        if let Some(s) = &self.diff_has {
            return Ok(diff.contains(s.as_str()));
        }
        if let Some(re) = &self.diff_has_re {
            let re = Regex::new(&format!("(?s){re}"))
                .with_context(|| format!("Invalid regex in category {:?}: {re}", self.name))?;
            return Ok(re.is_match(diff)?);
        }
        if let Some(strs) = &self.diff_has_any {
            return Ok(!diff.is_empty() && strs.iter().any(|s| diff.contains(s.as_str())));
        }
        if self.has_diff {
            return Ok(!diff.is_empty());
        }
        if self.catch_all {
            return Ok(true);
        }

        bail!("Category {:?} has no matcher configured", self.name)
    }
}

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

impl StatsConfigFile {
    /// Returns the categories for the given backend, with the universal
    /// `[backend.all]` entries appended after the backend-specific ones.
    pub fn categories_for<'a>(&'a self, backend: &str) -> Vec<&'a ErrorCategory> {
        let specific = self
            .backends
            .get(backend)
            .map(|b| b.categories.iter().collect::<Vec<_>>())
            .unwrap_or_default();

        let universal = self
            .backends
            .get("all")
            .map(|b| b.categories.iter().collect::<Vec<_>>())
            .unwrap_or_default();

        specific.into_iter().chain(universal).collect()
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

pub fn load(path: &Path) -> Result<StatsConfigFile> {
    let buf =
        fs::read_to_string(path).with_context(|| format!("Failed to read {:?}", path))?;
    let cfg = toml::from_str::<StatsConfigFile>(&buf)
        .with_context(|| format!("Failed to parse {:?}", path))?;
    Ok(cfg)
}

pub fn load_or_default(path: Option<&Path>) -> Result<StatsConfigFile> {
    let default_path = Path::new("/etc/rebuilderd-stats.conf");
    match path {
        Some(p) => load(p),
        None if default_path.exists() => load(default_path),
        None => Ok(StatsConfigFile::default()),
    }
}
