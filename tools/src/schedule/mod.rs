use crate::args::PkgsSync;
use glob::Pattern;
use rebuilderd_common::errors::*;
use reqwest::Client;
use std::fs;

pub async fn fetch_url_or_path(client: &Client, path: &str) -> Result<Vec<u8>> {
    let bytes = if path.starts_with("https://") || path.starts_with("http://") {
        info!("Downloading {:?}...", path);
        client.get(path)
            .send()
            .await?
            .bytes()
            .await?
            .to_vec()
    } else {
        info!("Reading {:?}...", path);
        fs::read(path)?
    };

    Ok(bytes)
}

pub trait Pkg {
    fn pkg_name(&self) -> &str;

    fn matches(&self, sync: &PkgsSync) -> bool {
        if self.match_name(&sync.excludes) {
            return false;
        }

        if sync.maintainers.is_empty() && sync.pkgs.is_empty() {
            true
        } else {
            self.from_maintainer(&sync.maintainers) || self.match_name(&sync.pkgs)
        }
    }

    fn from_maintainer(&self, maintainers: &[String]) -> bool;

    fn match_name(&self, patterns: &[Pattern]) -> bool {
        patterns.iter()
            .any(|p| p.matches(self.pkg_name()))
    }
}

pub mod archlinux;
pub mod debian;
pub mod tails;

#[cfg(test)]
mod tests {
    use crate::schedule::archlinux::ArchPkg;
    use super::*;

    struct Filter {
        maintainers: Vec<String>,
        pkgs: Vec<String>,
        excludes: Vec<String>,
    }

    fn to_patterns(patterns: Vec<String>) -> Vec<Pattern> {
        patterns.iter()
            .map(|f| Pattern::new(f).unwrap())
            .collect()
    }

    fn gen_filter(f: Filter) -> PkgsSync {
        PkgsSync {
            distro: "archlinux".to_string(),
            sync_method: None,
            suite: "community".to_string(),
            architectures: vec!["x86_64".to_string()],
            source: "https://ftp.halifax.rwth-aachen.de/archlinux/$repo/os/$arch".to_string(),
            releases: Vec::new(),

            print_json: false,
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
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: Vec::new(),
            excludes: Vec::new(),
        }));
        assert!(m);
    }

    #[test]
    fn maintainer_matches() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: vec!["kpcyrd <kpcyrd@archlinux.org>".to_string()],
            pkgs: Vec::new(),
            excludes: Vec::new(),
        }));
        assert!(m);
    }

    #[test]
    fn maintainer_does_not_match() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: vec!["Levente Polyak <anthraxx@archlinux.org>".to_string()],
            pkgs: Vec::new(),
            excludes: Vec::new(),
        }));
        assert!(!m);
    }

    #[test]
    fn pkg_name_matches() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: vec!["rebuilderd".to_string()],
            excludes: Vec::new(),
        }));
        assert!(m);
    }

    #[test]
    fn pkg_name_does_not_match() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: vec!["asdf".to_string()],
            excludes: Vec::new(),
        }));
        assert!(!m);
    }

    #[test]
    fn pkg_name_and_maintainer_matches() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: vec!["kpcyrd <kpcyrd@archlinux.org>".to_string()],
            pkgs: vec!["rebuilderd".to_string()],
            excludes: Vec::new(),
        }));
        assert!(m);
    }

    #[test]
    fn pkg_name_and_maintainer_does_not_match() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: vec!["Levente Polyak <anthraxx@archlinux.org>".to_string()],
            pkgs: vec!["linux-hardened".to_string()],
            excludes: Vec::new(),
        }));
        assert!(!m);
    }

    #[test]
    fn no_filter_but_excludes_match() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: Vec::new(),
            excludes: vec!["rebuilderd".to_string()],
        }));
        assert!(!m);
    }

    #[test]
    fn no_filter_and_no_excludes_match() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: Vec::new(),
            excludes: vec!["asdf".to_string()],
        }));
        assert!(m);
    }

    #[test]
    fn pkg_name_and_maintainer_match_and_no_excludes_match() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: vec!["kpcyrd <kpcyrd@archlinux.org>".to_string()],
            pkgs: vec!["rebuilderd".to_string()],
            excludes: vec!["asdf".to_string()],
        }));
        assert!(m);
    }

    #[test]
    fn pkg_name_and_maintainer_match_but_excludes_match() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: vec!["kpcyrd <kpcyrd@archlinux.org>".to_string()],
            pkgs: vec!["rebuilderd".to_string()],
            excludes: vec!["rebuilderd".to_string()],
        }));
        assert!(!m);
    }

    #[test]
    fn regular_string_matches_exact() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: vec!["rebuilderd".to_string()],
            excludes: Vec::new(),
        }));
        assert!(m);
    }

    #[test]
    fn regular_string_matches_only_exact() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: vec!["build".to_string()],
            excludes: Vec::new(),
        }));
        assert!(!m);
    }

    #[test]
    fn pattern_matches_prefix() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: vec!["*builderd".to_string()],
            excludes: Vec::new(),
        }));
        assert!(m);
    }

    #[test]
    fn pattern_matches_suffix() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: vec!["rebuild*".to_string()],
            excludes: Vec::new(),
        }));
        assert!(m);
    }

    #[test]
    fn pattern_matches_prefix_and_suffix() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: vec!["*build*".to_string()],
            excludes: Vec::new(),
        }));
        assert!(m);
    }

    #[test]
    fn pattern_matches_empty_string() {
        let m = gen_pkg().matches(&gen_filter(Filter {
            maintainers: Vec::new(),
            pkgs: vec!["rebuilder*".to_string()],
            excludes: Vec::new(),
        }));
        assert!(m);
    }
}
