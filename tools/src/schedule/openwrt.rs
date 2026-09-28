//! OpenWrt importer (`openwrt`): one job per target/subtarget, rebuilt
//! from source the way the buildbots build it. Such a build produces the
//! firmware images (profiles.json), the target-specific packages
//! (packages/Packages.bom.cdx.json) and the kernel modules
//! (kmods/<kernel>/Packages.bom.cdx.json), so all three become artifacts of the
//! same job.
//!
//! Every artifact records its target/subtarget as architecture, like the job,
//! and its kind as component, so `pkgs ls --architecture x86/64 --suite kmods`
//! selects one target's kmods.

use crate::args::PkgsSync;
use crate::schedule::fetch_url_or_path;
use rebuilderd_common::api::v1::{BinaryPackageReport, PackageReport, SourcePackageReport};
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use serde::Deserialize;
use std::collections::BTreeMap;

// Maps a release identifier (as written in rebuilderd-sync.conf) to the
// corresponding path under downloads.openwrt.org.
//
//   "SNAPSHOT" / "main"       -> snapshots/
//   "openwrt-24.10"           -> releases/24.10-SNAPSHOT/
//   "v24.10.0" / "24.10.0"    -> releases/24.10.0/
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

// Subset of OpenWrt's targets/<target>/<subtarget>/profiles.json we need: the
// release version + build code, the kernel (which names the kmods directory)
// and every device profile's image list.
#[derive(Debug, Deserialize)]
pub struct ProfilesJson {
    #[serde(default)]
    pub version_number: String,
    // The exact build identity (e.g. "r34845-193f1e3266"). version_number names
    // the release ("SNAPSHOT", "25.12.4") and is reused across every nightly
    // snapshot roll; version_code changes on each one. We fold it into the
    // source-package version so a new snapshot is seen as a new version and gets
    // re-triggered — otherwise the daemon dedups every roll to one version.
    #[serde(default)]
    pub version_code: String,
    pub linux_kernel: Option<LinuxKernel>,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

#[derive(Debug, Deserialize)]
pub struct LinuxKernel {
    pub version: String,
    pub release: String,
    pub vermagic: String,
}

impl LinuxKernel {
    // kmods/ keeps one directory per kernel build; this build's is named after
    // its kernel, e.g. "6.18.52-1-765b66c0498f1e672977ffa7bee0a0c8".
    fn kmods_dir(&self) -> String {
        format!("{}-{}-{}", self.version, self.release, self.vermagic)
    }
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

// Subset of the CycloneDX SBOM (Packages.bom.cdx.json) published next to the
// target packages and the kmods. Unlike index.json, which lists a library under
// its name without ABI version ("libatomic"), the SBOM carries the name the apk
// is published under ("libatomic1" -> libatomic1-14.4.0-r5.apk).
#[derive(Debug, Deserialize)]
pub struct Sbom {
    pub components: Vec<SbomComponent>,
}

#[derive(Debug, Deserialize)]
pub struct SbomComponent {
    pub name: String,
    pub version: String,
}

// Source-package version = release version + build code. version_number names
// the release; version_code pins the exact build. Snapshots keep the same
// version_number across nightly rolls, so without the code the daemon dedups
// every roll to one source-package version and never re-triggers; appending it
// makes each roll a distinct version. Falls back to the release name when a
// target omits version_number (metadata only — comparison uses artifact bytes).
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

// Component of the firmware images.
const FIRMWARE: &str = "firmware";
// Component of the target packages.
const PACKAGES: &str = "packages";
// Component of the kernel modules.
const KMODS: &str = "kmods";

// Add every apk listed in the SBOM under `dir_url` as an artifact.
async fn add_packages(
    http: &http::Client,
    dir_url: &str,
    component: &str,
    target: &str,
    artifacts: &mut BTreeMap<String, BinaryPackageReport>,
) -> Result<()> {
    let sbom_url = format!("{dir_url}/Packages.bom.cdx.json");
    let bytes = fetch_url_or_path(http, &sbom_url)
        .await
        .with_context(|| anyhow!("Failed to fetch {sbom_url}"))?;
    let sbom: Sbom =
        serde_json::from_slice(&bytes).with_context(|| anyhow!("Failed to parse {sbom_url}"))?;

    for package in &sbom.components {
        let filename = format!("{}-{}.apk", package.name, package.version);
        artifacts.insert(
            filename.clone(),
            BinaryPackageReport {
                name: package.name.clone(),
                version: package.version.clone(),
                component: Some(component.to_string()),
                architecture: target.to_string(),
                url: format!("{dir_url}/{filename}"),
            },
        );
    }
    info!("Loaded {} packages from {sbom_url}", sbom.components.len());

    Ok(())
}

// One rebuild = one (target, subtarget) built from source. So we emit a single
// source package per (target, subtarget) whose artifacts are all of that
// subtarget's images, target packages and kmods.
//
// `components` in the sync profile carry "<target>/<subtarget>" pairs (e.g.
// "x86/64").
pub async fn sync(http: &http::Client, sync: &PkgsSync) -> Result<Vec<PackageReport>> {
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

            // Keyed by filename: an image shared by multiple profiles is still
            // one artifact, and the worker matches rebuilt files by filename.
            let mut artifacts: BTreeMap<String, BinaryPackageReport> = BTreeMap::new();
            for profile in profiles.profiles.values() {
                for image in &profile.images {
                    artifacts
                        .entry(image.name.clone())
                        .or_insert_with(|| BinaryPackageReport {
                            name: image.name.clone(),
                            version: version.clone(),
                            component: Some(FIRMWARE.to_string()),
                            architecture: target_subtarget.to_string(),
                            url: format!("{base_url}/{}", image.name),
                        });
                }
            }
            info!(
                "Loaded {} images for {target_subtarget} from {profiles_url}",
                artifacts.len()
            );

            add_packages(
                http,
                &format!("{base_url}/packages"),
                PACKAGES,
                target_subtarget,
                &mut artifacts,
            )
            .await?;
            if let Some(kernel) = &profiles.linux_kernel {
                add_packages(
                    http,
                    &format!("{base_url}/kmods/{}", kernel.kmods_dir()),
                    KMODS,
                    target_subtarget,
                    &mut artifacts,
                )
                .await?;
            } else {
                warn!("No linux_kernel in {profiles_url}, skipping kmods");
            }

            reports.push(PackageReport {
                distribution: "openwrt".to_string(),
                release: Some(release.name().to_string()),
                // Use the target/subtarget as the job's "architecture". All
                // targets are cross-compiled on one x86_64 worker, but if every
                // target shared that build-host arch they'd also share the
                // daemon's (distribution, release, architecture) sync scope — so
                // syncing one target would mark all the others unseen and drop
                // their queued jobs. A distinct value per target gives each its
                // own scope, so targets sync independently (upstream rebuilds
                // them at different times). The worker advertises the "*"
                // wildcard to match them all.
                architecture: target_subtarget.to_string(),
                packages: vec![SourcePackageReport {
                    name: target_subtarget.to_string(),
                    version: version.clone(),
                    // The worker backend derives target/subtarget/release from
                    // this URL (see rebuilder-openwrt.sh).
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
    fn parse_sbom() {
        let bytes = br#"{
            "bomFormat": "CycloneDX",
            "specVersion": "1.4",
            "version": 1,
            "metadata": {"timestamp": "2026-09-22T14:00:05Z"},
            "components": [
                {"name": "kernel", "version": "6.18.52~8a2128798a21753379be49e778752d44-r1", "cpe": "cpe:/o:linux:linux_kernel", "type": "application"},
                {"name": "libatomic1", "version": "14.4.0-r5", "type": "library", "licenses": [{"license": {"name": "GPL-3.0-with-GCC-exception"}}]}
            ]
        }"#;
        let sbom: Sbom = serde_json::from_slice(bytes).unwrap();
        assert_eq!(sbom.components.len(), 2);
        assert_eq!(sbom.components[1].name, "libatomic1");
        assert_eq!(sbom.components[1].version, "14.4.0-r5");
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
            "linux_kernel": {"release": "1", "vermagic": "765b66c0498f1e672977ffa7bee0a0c8", "version": "6.18.52"},
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
        assert_eq!(p.version_number, "25.12.4");
        assert_eq!(p.version_code, "r28922-c2e2d9b245");
        assert_eq!(
            p.linux_kernel.unwrap().kmods_dir(),
            "6.18.52-1-765b66c0498f1e672977ffa7bee0a0c8"
        );
        assert_eq!(p.profiles["generic"].images.len(), 2);
        assert_eq!(
            p.profiles["generic"].images[0].name,
            "openwrt-25.12.4-x86-64-generic-squashfs-combined.img.gz"
        );
    }
}
