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

        if let Some(maintainer) = &self.maintainer
            && !matches_maintainer(pkg, maintainer)
        {
            return false;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::PkgsSync;
    use crate::schedule::archlinux::ArchPkg;
    use glob::Pattern;

    struct Filter {
        include: Vec<IncludeRule>,
        maintainers: Vec<String>,
        pkgs: &'static [&'static str],
        excludes: &'static [&'static str],
    }

    fn to_patterns(patterns: &[&str]) -> Vec<Pattern> {
        patterns.iter().map(|f| Pattern::new(f).unwrap()).collect()
    }

    fn gen_filter(f: Filter) -> PkgsSync {
        PkgsSync {
            distro: "archlinux".to_string(),
            sync_method: None,
            components: vec!["community".to_string()],
            architectures: vec!["x86_64".to_string()],
            source: "https://ftp.halifax.rwth-aachen.de/archlinux/$repo/os/$arch".to_string(),
            releases: Vec::new(),

            print_json: false,
            include: f.include,
            maintainers: f.maintainers,
            pkgs: to_patterns(f.pkgs),
            excludes: to_patterns(f.excludes),
        }
    }

    fn gen_pkg() -> ArchPkg {
        ArchPkg {
            name: "rebuilderd".to_string(),
            base: "rebuilderd".to_string(),
            filename: "rebuilderd-0.2.1-1-x86_64.pkg.tar.zst".to_string(),
            version: "0.2.1-1".to_string(),
            architecture: "x86_64".to_string(),
            packager: "kpcyrd <kpcyrd@archlinux.org>".to_string(),
        }
    }

    #[test]
    fn no_filter_always_matches() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn maintainer_matches() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: vec!["kpcyrd <kpcyrd@archlinux.org>".to_string()],
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn maintainer_does_not_match() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: vec!["Levente Polyak <anthraxx@archlinux.org>".to_string()],
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(!m);
    }

    #[test]
    fn pkg_name_matches() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &["rebuilderd"],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn pkg_name_does_not_match() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &["asdf"],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(!m);
    }

    #[test]
    fn pkg_name_and_maintainer_matches() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: vec!["kpcyrd <kpcyrd@archlinux.org>".to_string()],
                pkgs: &["rebuilderd"],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn pkg_name_and_maintainer_does_not_match() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: vec!["Levente Polyak <anthraxx@archlinux.org>".to_string()],
                pkgs: &["linux-hardened"],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(!m);
    }

    #[test]
    fn no_filter_but_excludes_match() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &["rebuilderd"],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(!m);
    }

    #[test]
    fn no_filter_and_no_excludes_match() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &["asdf"],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn pkg_name_and_maintainer_match_and_no_excludes_match() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: vec!["kpcyrd <kpcyrd@archlinux.org>".to_string()],
                pkgs: &["rebuilderd"],
                excludes: &["asdf"],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn pkg_name_and_maintainer_match_but_excludes_match() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: vec!["kpcyrd <kpcyrd@archlinux.org>".to_string()],
                pkgs: &["rebuilderd"],
                excludes: &["rebuilderd"],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(!m);
    }

    #[test]
    fn regular_string_matches_exact() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &["rebuilderd"],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn regular_string_matches_only_exact() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &["build"],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(!m);
    }

    #[test]
    fn pattern_matches_prefix() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &["*builderd"],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn pattern_matches_suffix() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &["rebuild*"],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn pattern_matches_prefix_and_suffix() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &["*build*"],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn pattern_matches_empty_string() {
        let m = matches(
            &gen_filter(Filter {
                include: Vec::new(),
                maintainers: Vec::new(),
                pkgs: &["rebuilderd*"],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn include_rule_matches_all_fields() {
        let m = matches(
            &gen_filter(Filter {
                include: vec![IncludeRule {
                    exclude: false,
                    component: Some("extra".to_string()),
                    binary_pkgs: to_patterns(&["rebuilderd"]),
                    source_pkgs: to_patterns(&["rebuilderd"]),
                    maintainer: Some("kpcyrd <kpcyrd@archlinux.org>".to_string()),
                }],
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn include_rule_first_match_wins() {
        let m = matches(
            &gen_filter(Filter {
                include: vec![
                    // This is an exclude=true rule
                    IncludeRule {
                        exclude: true,
                        component: None,
                        binary_pkgs: to_patterns(&["rebuilderd"]),
                        source_pkgs: Vec::new(),
                        maintainer: None,
                    },
                    // This is an exclude=false rule
                    IncludeRule {
                        exclude: false,
                        component: None,
                        binary_pkgs: to_patterns(&["rebuilderd"]),
                        source_pkgs: Vec::new(),
                        maintainer: None,
                    },
                ],
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(!m);
    }

    #[test]
    fn include_rules_default_to_deny_when_no_rule_matches() {
        let m = matches(
            &gen_filter(Filter {
                include: vec![IncludeRule {
                    exclude: false,
                    component: Some("community".to_string()),
                    binary_pkgs: to_patterns(&["linux"]),
                    source_pkgs: Vec::new(),
                    maintainer: None,
                }],
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(!m);
    }

    #[test]
    fn include_rules_fall_back_to_legacy_filters() {
        let m = matches(
            &gen_filter(Filter {
                include: vec![IncludeRule {
                    exclude: false,
                    component: Some("community".to_string()),
                    binary_pkgs: to_patterns(&["linux"]),
                    source_pkgs: Vec::new(),
                    maintainer: None,
                }],
                maintainers: vec!["kpcyrd <kpcyrd@archlinux.org>".to_string()],
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn include_rules_component_match() {
        let m = matches(
            &gen_filter(Filter {
                include: vec![IncludeRule {
                    exclude: false,
                    component: Some("core".to_string()),
                    binary_pkgs: Vec::new(),
                    source_pkgs: Vec::new(),
                    maintainer: None,
                }],
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "core",
        );
        assert!(m);
    }

    #[test]
    fn include_rules_component_mismatch() {
        let m = matches(
            &gen_filter(Filter {
                include: vec![IncludeRule {
                    exclude: false,
                    component: Some("core".to_string()),
                    binary_pkgs: Vec::new(),
                    source_pkgs: Vec::new(),
                    maintainer: None,
                }],
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(!m);
    }

    #[test]
    fn include_rules_maintainer_matches() {
        let m = matches(
            &gen_filter(Filter {
                include: vec![IncludeRule {
                    exclude: false,
                    component: None,
                    binary_pkgs: Vec::new(),
                    source_pkgs: Vec::new(),
                    maintainer: Some("kpcyrd".to_string()),
                }],
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn include_rules_maintainer_does_not_match() {
        let m = matches(
            &gen_filter(Filter {
                include: vec![IncludeRule {
                    exclude: false,
                    component: None,
                    binary_pkgs: Vec::new(),
                    source_pkgs: Vec::new(),
                    maintainer: Some("Levente Polyak".to_string()),
                }],
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(!m);
    }

    #[test]
    fn include_rules_binary_pkg_wildcard() {
        let m = matches(
            &gen_filter(Filter {
                include: vec![IncludeRule {
                    exclude: false,
                    component: None,
                    binary_pkgs: to_patterns(&["*"]),
                    source_pkgs: Vec::new(),
                    maintainer: None,
                }],
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }

    #[test]
    fn include_rules_source_pkg_wildcard() {
        let m = matches(
            &gen_filter(Filter {
                include: vec![IncludeRule {
                    exclude: false,
                    component: None,
                    binary_pkgs: Vec::new(),
                    source_pkgs: to_patterns(&["*"]),
                    maintainer: None,
                }],
                maintainers: Vec::new(),
                pkgs: &[],
                excludes: &[],
            }),
            &gen_pkg(),
            "extra",
        );
        assert!(m);
    }
}
