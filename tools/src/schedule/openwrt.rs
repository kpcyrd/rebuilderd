//! OpenWrt importers for both datasets, which share the same download layout
//! and release→path mapping:
//!   - `openwrt-package` ([`sync_package`]): apk feeds (index.json), per arch + component
//!   - `openwrt-image`   ([`sync_image`]):   firmware images per target/subtarget (profiles.json)

use crate::args::PkgsSync;
use crate::rules;
use crate::schedule::{Pkg, fetch_url_or_path};
use rebuilderd_common::api::v1::{BinaryPackageReport, PackageReport, SourcePackageReport};
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Maps a release identifier (as written in rebuilderd-sync.conf) to the
// corresponding path under downloads.openwrt.org and the docker SDK tag suffix.
//
//   "SNAPSHOT" / "main"       -> snapshots/                  tag: -SNAPSHOT
//   "openwrt-24.10"           -> releases/24.10-SNAPSHOT/    tag: -openwrt-24.10
//   "v24.10.0" / "24.10.0"    -> releases/24.10.0/           tag: -v24.10.0 (or -24.10.0)
fn release_to_path(release: &str) -> String {
    match release {
        "SNAPSHOT" | "main" => "snapshots".to_string(),
        s if s.starts_with("openwrt-") => {
            format!("releases/{}-SNAPSHOT", &s["openwrt-".len()..])
        }
        s => {
            let trimmed = s.strip_prefix('v').unwrap_or(s);
            format!("releases/{trimmed}")
        }
    }
}

// ===========================================================================
// apk packages (openwrt-package)
// ===========================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexJson {
    pub version: u32,
    pub architecture: String,
    pub packages: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct OpenwrtPkg {
    pub name: String,
    pub version: String,
    pub architecture: String,
}

impl Pkg for OpenwrtPkg {
    fn binary_pkg_name(&self) -> &str {
        &self.name
    }

    // apk packages aren't grouped under a source package here; each is its own.
    fn source_pkg_name(&self) -> Option<&str> {
        None
    }

    // index.json carries no maintainer/packager field.
    fn maintainers(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(std::iter::empty())
    }
}

// Fetch and parse a feed's index.json, warning (but not failing) on an
// architecture mismatch between the index and the configured arch.
async fn fetch_index(http: &http::Client, base_url: &str, arch: &str) -> Result<IndexJson> {
    let index_url = format!("{base_url}/index.json");

    let bytes = fetch_url_or_path(http, &index_url)
        .await
        .with_context(|| anyhow!("Failed to fetch {index_url}"))?;

    let index: IndexJson =
        serde_json::from_slice(&bytes).with_context(|| anyhow!("Failed to parse {index_url}"))?;

    if index.architecture != *arch {
        warn!(
            "Architecture mismatch in {index_url}: index says {:?}, configured {:?} — using configured value",
            index.architecture, arch
        );
    }

    info!("Loaded {} packages from {index_url}", index.packages.len());

    Ok(index)
}

pub async fn sync_package(http: &http::Client, sync: &PkgsSync) -> Result<Vec<PackageReport>> {
    let mut reports = Vec::new();

    for release in &sync.releases {
        let base = release
            .source(&sync.source)
            .trim_end_matches('/')
            .to_string();
        let rel_path = release_to_path(release.name());

        for arch in &sync.architectures {
            for component in &sync.components {
                let base_url = format!("{base}/{rel_path}/packages/{arch}/{component}");

                let index = fetch_index(http, &base_url, arch).await?;

                let mut report = PackageReport {
                    distribution: "openwrt-package".to_string(),
                    release: Some(release.name().to_string()),
                    architecture: arch.clone(),
                    packages: Vec::new(),
                };

                for (name, version) in &index.packages {
                    let pkg = OpenwrtPkg {
                        name: name.clone(),
                        version: version.clone(),
                        architecture: arch.clone(),
                    };
                    if !rules::matches(sync, &pkg, component) {
                        continue;
                    }

                    let filename = format!("{name}-{version}.apk");
                    let url = format!("{base_url}/{filename}");

                    let artifact = BinaryPackageReport {
                        name: name.clone(),
                        version: version.clone(),
                        component: Some(component.clone()),
                        architecture: arch.clone(),
                        url: url.clone(),
                    };

                    report.packages.push(SourcePackageReport {
                        name: name.clone(),
                        version: version.clone(),
                        url,
                        artifacts: vec![artifact],
                    });
                }

                reports.push(report);
            }
        }
    }

    Ok(reports)
}

// ===========================================================================
// firmware images (openwrt-image)
// ===========================================================================

// Subset of OpenWrt's targets/<target>/<subtarget>/profiles.json we need: the
// package arch, the release version + build code, and every device profile's
// image list.
#[derive(Debug, Deserialize)]
pub struct ProfilesJson {
    pub arch_packages: String,
    #[serde(default)]
    pub version_number: String,
    // The exact build identity (e.g. "r34845-193f1e3266"). version_number names
    // the release ("SNAPSHOT", "25.12.4") and is reused across every nightly
    // snapshot roll; version_code changes on each one. We fold it into the
    // source-package version so a new snapshot is seen as a new version and gets
    // re-triggered — otherwise the daemon dedups every roll to one version.
    #[serde(default)]
    pub version_code: String,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

#[derive(Debug, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub images: Vec<Image>,
}

#[derive(Debug, Deserialize)]
pub struct Image {
    pub name: String,
}

// Source-package version = release version + build code. version_number names
// the release; version_code pins the exact build. Snapshots keep the same
// version_number across nightly rolls, so without the code the daemon dedups
// every roll to one source-package version and never re-triggers; appending it
// makes each roll a distinct version. Falls back to the release name when a
// target omits version_number (metadata only — comparison uses image bytes).
fn build_version(version_number: &str, version_code: &str, release_name: &str) -> String {
    let release_version = if version_number.is_empty() {
        release_name.trim_start_matches('v').to_string()
    } else {
        version_number.to_string()
    };
    if version_code.is_empty() {
        release_version
    } else {
        format!("{release_version}+{version_code}")
    }
}

// One firmware rebuild = one (target, subtarget) built from source, producing
// every device profile's images. So we emit a single source package per
// (target, subtarget) whose artifacts are all of that subtarget's image files.
//
// `components` in the sync profile carry "<target>/<subtarget>" pairs (e.g.
// "x86/64") — the firmware analogue of a package feed.
pub async fn sync_image(http: &http::Client, sync: &PkgsSync) -> Result<Vec<PackageReport>> {
    let mut reports = Vec::new();

    for release in &sync.releases {
        let base = release
            .source(&sync.source)
            .trim_end_matches('/')
            .to_string();
        let rel_path = release_to_path(release.name());

        for component in &sync.components {
            let target_subtarget = component.trim_matches('/');
            let base_url = format!("{base}/{rel_path}/targets/{target_subtarget}");
            let profiles_url = format!("{base_url}/profiles.json");

            let bytes = fetch_url_or_path(http, &profiles_url)
                .await
                .with_context(|| anyhow!("Failed to fetch {profiles_url}"))?;
            let profiles: ProfilesJson = serde_json::from_slice(&bytes)
                .with_context(|| anyhow!("Failed to parse {profiles_url}"))?;

            let version = build_version(
                &profiles.version_number,
                &profiles.version_code,
                release.name(),
            );

            // Collect every image across every profile, deduped by filename
            // (a file shared by multiple profiles is still one artifact).
            let mut artifacts: BTreeMap<String, BinaryPackageReport> = BTreeMap::new();
            for profile in profiles.profiles.values() {
                for image in &profile.images {
                    artifacts
                        .entry(image.name.clone())
                        .or_insert_with(|| BinaryPackageReport {
                            name: image.name.clone(),
                            version: version.clone(),
                            component: Some(target_subtarget.to_string()),
                            architecture: profiles.arch_packages.clone(),
                            url: format!("{base_url}/{}", image.name),
                        });
                }
            }

            info!(
                "Loaded {} images for {target_subtarget} from {profiles_url}",
                artifacts.len()
            );

            reports.push(PackageReport {
                distribution: "openwrt-image".to_string(),
                release: Some(release.name().to_string()),
                // Use the target/subtarget as the job's "architecture". Images
                // are all cross-compiled on one x86_64 worker, but if every
                // target shared that build-host arch they'd also share the
                // daemon's (distribution, release, architecture) sync scope — so
                // syncing one target would mark all the others unseen and drop
                // their queued jobs. A distinct value per target gives each its
                // own scope, so targets sync independently (upstream rebuilds
                // them at different times). The image worker advertises the "*"
                // wildcard to match them all; the real target arch is recorded
                // on each image artifact (`arch_packages`).
                architecture: target_subtarget.to_string(),
                packages: vec![SourcePackageReport {
                    name: target_subtarget.to_string(),
                    version: version.clone(),
                    // The worker backend derives target/subtarget/release from
                    // this URL (see rebuilder-openwrt-image.sh).
                    url: profiles_url.clone(),
                    artifacts: artifacts.into_values().collect(),
                }],
            });
        }
    }

    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_path_snapshot() {
        assert_eq!(release_to_path("SNAPSHOT"), "snapshots");
        assert_eq!(release_to_path("main"), "snapshots");
    }

    #[test]
    fn release_path_branch() {
        assert_eq!(release_to_path("openwrt-24.10"), "releases/24.10-SNAPSHOT");
    }

    #[test]
    fn release_path_tag() {
        assert_eq!(release_to_path("v24.10.0"), "releases/24.10.0");
        assert_eq!(release_to_path("24.10.0"), "releases/24.10.0");
    }

    #[test]
    fn apk_filename_format() {
        // Alpine apk convention: <name>-<version>.apk, no arch suffix.
        let name = "tmate";
        let version = "2.4.0-r3";
        let filename = format!("{name}-{version}.apk");
        assert_eq!(filename, "tmate-2.4.0-r3.apk");
    }

    #[test]
    fn parse_index_json() {
        let bytes = br#"{
            "version": 2,
            "architecture": "x86_64",
            "packages": {
                "tmate": "2.4.0-r1",
                "acme-common": "1.5.2"
            }
        }"#;
        let index: IndexJson = serde_json::from_slice(bytes).unwrap();
        assert_eq!(index.version, 2);
        assert_eq!(index.architecture, "x86_64");
        assert_eq!(index.packages.get("tmate"), Some(&"2.4.0-r1".to_string()));
        assert_eq!(
            index.packages.get("acme-common"),
            Some(&"1.5.2".to_string())
        );
    }

    #[test]
    fn version_combines_number_and_code() {
        assert_eq!(
            build_version("SNAPSHOT", "r34845-193f1e3266", "SNAPSHOT"),
            "SNAPSHOT+r34845-193f1e3266"
        );
        assert_eq!(
            build_version("25.12.4", "r28922-c2e2d9b245", "v25.12.4"),
            "25.12.4+r28922-c2e2d9b245"
        );
    }

    #[test]
    fn version_falls_back_without_code() {
        // No version_code -> bare release version, no "+" suffix.
        assert_eq!(build_version("25.12.4", "", "v25.12.4"), "25.12.4");
        // No version_number either -> derived from the release name.
        assert_eq!(build_version("", "", "v25.12.4"), "25.12.4");
    }

    #[test]
    fn parse_profiles_json() {
        let bytes = br#"{
            "arch_packages": "x86_64",
            "version_number": "25.12.4",
            "version_code": "r28922-c2e2d9b245",
            "profiles": {
                "generic": {
                    "images": [
                        {"name": "openwrt-25.12.4-x86-64-generic-squashfs-combined.img.gz", "sha256": "abc", "type": "combined"},
                        {"name": "openwrt-25.12.4-x86-64-generic-kernel.bin", "type": "kernel"}
                    ]
                }
            }
        }"#;
        let p: ProfilesJson = serde_json::from_slice(bytes).unwrap();
        assert_eq!(p.arch_packages, "x86_64");
        assert_eq!(p.version_number, "25.12.4");
        assert_eq!(p.version_code, "r28922-c2e2d9b245");
        assert_eq!(p.profiles["generic"].images.len(), 2);
        assert_eq!(
            p.profiles["generic"].images[0].name,
            "openwrt-25.12.4-x86-64-generic-squashfs-combined.img.gz"
        );
    }
}
