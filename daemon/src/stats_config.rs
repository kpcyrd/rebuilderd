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
    /// Pre-compile any regex patterns in this category.
    /// Call once per collect_stats run, not once per package.
    pub fn compile(&self) -> Result<CompiledCategory<'_>> {
        CompiledCategory::new(self)
    }
}

/// `ErrorCategory` with its regex fields pre-compiled.
/// Create via `ErrorCategory::compile()` before the per-package matching loop.
pub struct CompiledCategory<'a> {
    pub inner: &'a ErrorCategory,
    log_has_re: Option<Regex>,
    diff_has_re: Option<Regex>,
}

impl<'a> CompiledCategory<'a> {
    fn new(cat: &'a ErrorCategory) -> Result<Self> {
        // (?s) enables DOTALL so '.' matches newlines, mirroring Python's re.DOTALL.
        let compile = |re: &str| {
            Regex::new(&format!("(?s){re}"))
                .with_context(|| format!("Invalid regex in category {:?}: {re}", cat.name))
        };
        let log_has_re = cat.log_has_re.as_deref().map(compile).transpose()?;
        let diff_has_re = cat.diff_has_re.as_deref().map(compile).transpose()?;
        Ok(Self {
            inner: cat,
            log_has_re,
            diff_has_re,
        })
    }

    pub fn matches(&self, log: &str, diff: &str, architecture: Option<&str>) -> Result<bool> {
        let cat = self.inner;

        if let Some(arch) = architecture {
            if cat
                .exclude_architectures
                .iter()
                .any(|x| arch.starts_with(x.as_str()))
            {
                return Ok(false);
            }
        }

        // Helper: substitute {arch} with the actual architecture (or "" if unknown).
        let sub = |s: &str| -> String { s.replace("{arch}", architecture.unwrap_or("")) };

        if let Some(s) = &cat.log_has {
            return Ok(log.contains(sub(s).as_str()));
        }
        if let Some(re) = &self.log_has_re {
            return Ok(re.is_match(log)?);
        }
        if let Some(strs) = &cat.log_has_any {
            return Ok(strs.iter().any(|s| log.contains(sub(s).as_str())));
        }
        if let Some(strs) = &cat.log_has_all {
            return Ok(strs.iter().all(|s| log.contains(sub(s).as_str())));
        }
        if let Some(s) = &cat.diff_has {
            return Ok(diff.contains(sub(s).as_str()));
        }
        if let Some(re) = &self.diff_has_re {
            return Ok(re.is_match(diff)?);
        }
        if let Some(strs) = &cat.diff_has_any {
            return Ok(!diff.is_empty() && strs.iter().any(|s| diff.contains(sub(s).as_str())));
        }
        if cat.has_diff {
            return Ok(!diff.is_empty());
        }
        if cat.catch_all {
            return Ok(true);
        }

        bail!("Category {:?} has no matcher configured", cat.name)
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
    let buf = fs::read_to_string(path).with_context(|| format!("Failed to read {:?}", path))?;
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
