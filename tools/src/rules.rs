use crate::args::PkgsSync;
use crate::schedule::Pkg;
use serde::Deserialize;
use serde_with::{DisplayFromStr, serde_as};

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct IncludeRule {
    #[serde(default)]
    pub exclude: bool,
    pub component: Option<String>,
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub binary_pkgs: Vec<glob::Pattern>,
    #[serde(default)]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub source_pkgs: Vec<glob::Pattern>,
    #[serde(default)]
    pub maintainer: Option<String>,
}

impl IncludeRule {
    pub fn matches(&self, pkg: &dyn Pkg, component: &str) -> bool {
        if self.component.is_some() && self.component.as_deref() != Some(component) {
            return false;
        }

        if !self.binary_pkgs.is_empty()
            && !self
                .binary_pkgs
                .iter()
                .any(|p| p.matches(pkg.binary_pkg_name()))
        {
            return false;
        }

        if !self.source_pkgs.is_empty() {
            let Some(source_pkg_name) = pkg.source_pkg_name() else {
                return false;
            };

            if !self.source_pkgs.iter().any(|p| p.matches(source_pkg_name)) {
                return false;
            }
        }

        if let Some(maintainer) = &self.maintainer {
            if !matches_maintainer(pkg, maintainer) {
                return false;
            }
        }

        true
    }
}

/// Apply the older filter rules, as well as the new include= system
pub fn matches(sync: &PkgsSync, pkg: &dyn Pkg, component: &str) -> bool {
    if matches_name(pkg.binary_pkg_name(), &sync.excludes) {
        return false;
    }

    for rule in &sync.include {
        if rule.matches(pkg, component) {
            return !rule.exclude;
        }
    }

    if sync.include.is_empty() && sync.maintainers.is_empty() && sync.pkgs.is_empty() {
        true
    } else {
        matches_maintainers(pkg, &sync.maintainers)
            || matches_name(pkg.binary_pkg_name(), &sync.pkgs)
    }
}

/// Match if intersection of maintainers and filter is not empty
fn matches_maintainers(pkg: &dyn Pkg, filter: &[String]) -> bool {
    filter.iter().any(|filter| matches_maintainer(pkg, filter))
}

/// Match if any maintainer starts with the filter
fn matches_maintainer(pkg: &dyn Pkg, filter: &str) -> bool {
    pkg.maintainers()
        .any(|maintainer| maintainer.starts_with(filter))
}

/// Match if any of the patterns matches the name
fn matches_name(name: &str, patterns: &[glob::Pattern]) -> bool {
    patterns.iter().any(|p| p.matches(name))
}
