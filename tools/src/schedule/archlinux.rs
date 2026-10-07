use crate::args::PkgsSync;
use crate::decompress;
use crate::rules;
use crate::schedule::{Pkg, fetch_url_or_path};
use nom::bytes::complete::take_till;
use rebuilderd_common::api::v1::{BinaryPackageReport, PackageReport, SourcePackageReport};
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::io::prelude::*;
use std::iter;
use tar::{Archive, EntryType};

fn mirror_to_url(mut mirror: &str, repo: &str, arch: &str, file: &str) -> Result<String> {
    let mut url = String::new();

    loop {
        let (s, txt) = take_till::<_, _, ()>(|c| c == '$')(mirror).unwrap();
        url.push_str(txt);
        if s.is_empty() {
            break;
        }
        let (s, var) = take_till::<_, _, ()>(|c| c == '/')(s).unwrap();
        match var {
            "$repo" => url.push_str(repo),
            "$arch" => url.push_str(arch),
            _ => bail!("Unrecognized variable: {:?}", var),
        }
        mirror = s;
    }

    if !url.ends_with('/') {
        url.push('/');
    }
    url.push_str(file);

    Ok(url)
}

#[derive(Debug, Clone)]
pub struct ArchPkg {
    pub name: String,
    pub base: String,
    pub filename: String,
    pub version: String,
    pub architecture: String,
    pub packager: String,
}

impl Pkg for ArchPkg {
    fn binary_pkg_name(&self) -> &str {
        &self.name
    }

    fn source_pkg_name(&self) -> Option<&str> {
        Some(&self.base)
    }

    fn maintainers(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(iter::once(self.packager.as_str()))
    }
}

#[derive(Debug, Default)]
pub struct NewPkg {
    name: Vec<String>,
    base: Vec<String>,
    filename: Vec<String>,
    version: Vec<String>,
    architecture: Vec<String>,
    packager: Vec<String>,
}

impl TryInto<ArchPkg> for NewPkg {
    type Error = Error;

    fn try_into(self: NewPkg) -> Result<ArchPkg> {
        Ok(ArchPkg {
            name: self
                .name
                .first()
                .ok_or_else(|| anyhow!("Missing pkg name field"))?
                .to_string(),
            base: self
                .base
                .first()
                .ok_or_else(|| anyhow!("Missing pkg base field"))?
                .to_string(),
            filename: self
                .filename
                .first()
                .ok_or_else(|| anyhow!("Missing filename field"))?
                .to_string(),
            version: self
                .version
                .first()
                .ok_or_else(|| anyhow!("Missing version field"))?
                .to_string(),
            architecture: self
                .architecture
                .first()
                .ok_or_else(|| anyhow!("Missing architecture field"))?
                .to_string(),
            packager: self
                .packager
                .first()
                .ok_or_else(|| anyhow!("Missing packager field"))?
                .to_string(),
        })
    }
}

pub fn extract_pkgs(bytes: &[u8]) -> Result<Vec<ArchPkg>> {
    let comp = decompress::detect_compression(bytes);
    let tar = decompress::stream(comp, bytes)?;
    let mut archive = Archive::new(tar);

    let mut pkgs = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.header().entry_type() == EntryType::Regular {
            let mut pkg = NewPkg::default();

            let mut content = String::new();
            entry.read_to_string(&mut content)?;

            let mut iter = content.split('\n');
            while let Some(key) = iter.next() {
                let mut values = Vec::new();
                for value in &mut iter {
                    if !value.is_empty() {
                        values.push(value.to_string());
                    } else {
                        break;
                    }
                }

                match key {
                    "%FILENAME%" => pkg.filename = values,
                    "%NAME%" => pkg.name = values,
                    "%BASE%" => pkg.base = values,
                    "%VERSION%" => pkg.version = values,
                    "%ARCH%" => pkg.architecture = values,
                    "%PACKAGER%" => pkg.packager = values,
                    _ => (),
                }
            }

            pkgs.push(pkg.try_into()?);
        }
    }

    Ok(pkgs)
}

#[derive(Debug, Default)]
pub struct BuildGroups {
    bases: BTreeMap<String, SourcePackageReport>,
}

impl BuildGroups {
    pub fn add(&mut self, source: &str, arch: &str, component: String, pkg: ArchPkg) -> Result<()> {
        let url = mirror_to_url(source, &component, arch, &pkg.filename)?;
        let artifact = BinaryPackageReport {
            name: pkg.name,
            version: pkg.version.clone(),
            component: Some(component),
            architecture: pkg.architecture,
            url: url.clone(),
        };

        if let Some(group) = self.bases.get_mut(&pkg.base) {
            // TODO: multiple architectures could have the exact same package with arch=any

            // Ensure the build input url is stable, regardless of the insert order
            if url < group.url {
                group.url = url;
            }

            // Add this package to artifact list
            group.artifacts.push(artifact);
            group.artifacts.sort();
        } else {
            let mut group = SourcePackageReport {
                name: pkg.base.clone(),
                version: pkg.version.clone(),
                url: url.clone(), // use first artifact's url as the source URL for now
                artifacts: Vec::new(),
            };

            group.artifacts.push(artifact);
            self.bases.insert(pkg.base, group);
        }

        Ok(())
    }

    pub fn into_vec(self) -> Vec<SourcePackageReport> {
        self.bases.into_values().collect()
    }
}

pub async fn sync(http: &http::Client, sync: &PkgsSync) -> Result<Vec<PackageReport>> {
    let source = if sync.source.ends_with(".db") {
        warn!(
            "Detected legacy configuration for source, use the new format instead: https://mirrors.kernel.org/archlinux/$repo/os/$arch"
        );
        "https://mirrors.kernel.org/archlinux/$repo/os/$arch"
    } else {
        &sync.source
    };

    let mut reports = Vec::new();
    for arch in &sync.architectures {
        let mut report = PackageReport {
            distribution: "archlinux".to_string(),
            release: None,
            architecture: arch.clone(),
            packages: Vec::new(),
        };

        let mut bases = BuildGroups::default();

        for component in &sync.components {
            let db = mirror_to_url(source, component, arch, &format!("{}.db", component))?;
            let bytes = fetch_url_or_path(http, &db).await?;

            info!("Parsing index ({} bytes)...", bytes.len());
            for pkg in extract_pkgs(&bytes)? {
                if !rules::matches(sync, &pkg, component) {
                    continue;
                }

                bases.add(source, arch, component.clone(), pkg)?;
            }
        }

        report.packages = bases.into_vec();
        reports.push(report);
    }

    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mirror_to_url() {
        let url = mirror_to_url(
            "https://ftp.halifax.rwth-aachen.de/archlinux/$repo/os/$arch",
            "core",
            "x86_64",
            "core.db",
        )
        .unwrap();
        assert_eq!(
            url,
            "https://ftp.halifax.rwth-aachen.de/archlinux/core/os/x86_64/core.db"
        );
    }

    #[test]
    fn test_simple_build_group() {
        let mut groups = BuildGroups::default();
        groups
            .add(
                "https://mirrors.kernel.org/archlinux/$repo/os/$arch",
                "x86_64",
                "core".to_string(),
                ArchPkg {
                    name: "bash".to_string(),
                    base: "bash".to_string(),
                    filename: "bash-5.3.15-1-x86_64.pkg.tar.zst".to_string(),
                    version: "5.3.15-1".to_string(),
                    architecture: "x86_64".to_string(),
                    packager: "John Doe <john.doe@example.com>".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            groups.into_vec(),
            vec![SourcePackageReport {
                name: "bash".to_string(),
                version: "5.3.15-1".to_string(),
                url: "https://mirrors.kernel.org/archlinux/core/os/x86_64/bash-5.3.15-1-x86_64.pkg.tar.zst".to_string(),
                artifacts: vec![BinaryPackageReport {
                    name: "bash".to_string(),
                    version: "5.3.15-1".to_string(),
                    component: Some("core".to_string()),
                    architecture: "x86_64".to_string(),
                    url: "https://mirrors.kernel.org/archlinux/core/os/x86_64/bash-5.3.15-1-x86_64.pkg.tar.zst".to_string(),
                }],
            }]
        );
    }

    #[test]
    fn test_two_build_groups() {
        let mut groups = BuildGroups::default();

        let source = "https://mirrors.kernel.org/archlinux/$repo/os/$arch";
        let arch = "x86_64";
        let component = "core";

        groups
            .add(
                source,
                arch,
                component.to_string(),
                ArchPkg {
                    name: "bash".to_string(),
                    base: "bash".to_string(),
                    filename: "bash-5.3.15-1-x86_64.pkg.tar.zst".to_string(),
                    version: "5.3.15-1".to_string(),
                    architecture: "x86_64".to_string(),
                    packager: "John Doe <john.doe@example.com>".to_string(),
                },
            )
            .unwrap();

        groups
            .add(
                source,
                arch,
                component.to_string(),
                ArchPkg {
                    name: "btrfs-progs".to_string(),
                    base: "btrfs-progs".to_string(),
                    filename: "btrfs-progs-7.1-1-x86_64.pkg.tar.zst".to_string(),
                    version: "7.1-1".to_string(),
                    architecture: "x86_64".to_string(),
                    packager: "John Doe <john.doe@example.com>".to_string(),
                },
            )
            .unwrap();

        assert_eq!(
            groups.into_vec(),
            &[
                SourcePackageReport {
                    name: "bash".to_string(),
                    version: "5.3.15-1".to_string(),
                    url: "https://mirrors.kernel.org/archlinux/core/os/x86_64/bash-5.3.15-1-x86_64.pkg.tar.zst".to_string(),
                    artifacts: vec![BinaryPackageReport {
                        name: "bash".to_string(),
                        version: "5.3.15-1".to_string(),
                        component: Some("core".to_string()),
                        architecture: "x86_64".to_string(),
                        url: "https://mirrors.kernel.org/archlinux/core/os/x86_64/bash-5.3.15-1-x86_64.pkg.tar.zst".to_string(),
                    }],
                },
                SourcePackageReport {
                    name: "btrfs-progs".to_string(),
                    version: "7.1-1".to_string(),
                    url: "https://mirrors.kernel.org/archlinux/core/os/x86_64/btrfs-progs-7.1-1-x86_64.pkg.tar.zst".to_string(),
                    artifacts: vec![BinaryPackageReport {
                        name: "btrfs-progs".to_string(),
                        version: "7.1-1".to_string(),
                        component: Some("core".to_string()),
                        architecture: "x86_64".to_string(),
                        url: "https://mirrors.kernel.org/archlinux/core/os/x86_64/btrfs-progs-7.1-1-x86_64.pkg.tar.zst".to_string(),
                    }],
                }
            ]
        );
    }

    #[test]
    fn test_split_pkg() {
        let source = "https://mirrors.kernel.org/archlinux/$repo/os/$arch";
        let arch = "x86_64";
        let component = "extra";

        // 3 packages in variables since we are going to use them twice
        let bitcoin_daemon = ArchPkg {
            name: "bitcoin-daemon".to_string(),
            base: "bitcoin".to_string(),
            filename: "bitcoin-daemon-31.1-3-x86_64.pkg.tar.zst".to_string(),
            version: "31.1-3".to_string(),
            architecture: "x86_64".to_string(),
            packager: "John Doe <john.doe@example.com>".to_string(),
        };

        let bitcoin_tx = ArchPkg {
            name: "bitcoin-tx".to_string(),
            base: "bitcoin".to_string(),
            filename: "bitcoin-tx-31.1-3-x86_64.pkg.tar.zst".to_string(),
            version: "31.1-3".to_string(),
            architecture: "x86_64".to_string(),
            packager: "John Doe <john.doe@example.com>".to_string(),
        };

        let bitcoin_qt = ArchPkg {
            name: "bitcoin-qt".to_string(),
            base: "bitcoin".to_string(),
            filename: "bitcoin-qt-31.1-3-x86_64.pkg.tar.zst".to_string(),
            version: "31.1-3".to_string(),
            architecture: "x86_64".to_string(),
            packager: "John Doe <john.doe@example.com>".to_string(),
        };

        // Setup groups and convert to list
        let mut groups = BuildGroups::default();
        for pkg in [&bitcoin_daemon, &bitcoin_tx, &bitcoin_qt] {
            groups
                .add(source, arch, component.to_string(), pkg.clone())
                .unwrap();
        }
        let packages = groups.into_vec();

        assert_eq!(packages, &[SourcePackageReport {
                name: "bitcoin".to_string(),
                version: "31.1-3".to_string(),
                url: "https://mirrors.kernel.org/archlinux/extra/os/x86_64/bitcoin-daemon-31.1-3-x86_64.pkg.tar.zst".to_string(),
                artifacts: vec![
                    BinaryPackageReport {
                        name: "bitcoin-daemon".to_string(),
                        version: "31.1-3".to_string(),
                        component: Some("extra".to_string()),
                        architecture: "x86_64".to_string(),
                        url: "https://mirrors.kernel.org/archlinux/extra/os/x86_64/bitcoin-daemon-31.1-3-x86_64.pkg.tar.zst".to_string(),
                    },
                    BinaryPackageReport {
                        name: "bitcoin-qt".to_string(),
                        version: "31.1-3".to_string(),
                        component: Some("extra".to_string()),
                        architecture: "x86_64".to_string(),
                        url: "https://mirrors.kernel.org/archlinux/extra/os/x86_64/bitcoin-qt-31.1-3-x86_64.pkg.tar.zst".to_string(),
                    },
                    BinaryPackageReport {
                        name: "bitcoin-tx".to_string(),
                        version: "31.1-3".to_string(),
                        component: Some("extra".to_string()),
                        architecture: "x86_64".to_string(),
                        url: "https://mirrors.kernel.org/archlinux/extra/os/x86_64/bitcoin-tx-31.1-3-x86_64.pkg.tar.zst".to_string(),
                    },
                ],
        }]);

        // Ensure add-order doesn't affect resolved groups
        let mut groups = BuildGroups::default();
        for pkg in [bitcoin_qt, bitcoin_tx, bitcoin_daemon] {
            groups
                .add(source, arch, component.to_string(), pkg)
                .unwrap();
        }
        let packages2 = groups.into_vec();
        assert_eq!(packages, packages2);
    }

    #[test]
    fn test_groups_core_and_core_testing_regression_271() {
        // https://github.com/kpcyrd/rebuilderd/issues/271

        let mut groups = BuildGroups::default();

        let source = "https://mirrors.kernel.org/archlinux/$repo/os/$arch";
        let arch = "x86_64";

        groups
            .add(
                source,
                arch,
                "core".to_string(),
                ArchPkg {
                    name: "perl".to_string(),
                    base: "perl".to_string(),
                    filename: "perl-5.42.2-1-x86_64.pkg.tar.zst".to_string(),
                    version: "5.42.2-1".to_string(),
                    architecture: "x86_64".to_string(),
                    packager: "John Doe <john.doe@example.com>".to_string(),
                },
            )
            .unwrap();

        groups
            .add(
                source,
                arch,
                "core-testing".to_string(),
                ArchPkg {
                    name: "perl".to_string(),
                    base: "perl".to_string(),
                    filename: "perl-5.42.3-1-x86_64.pkg.tar.zst".to_string(),
                    version: "5.42.3-1".to_string(),
                    architecture: "x86_64".to_string(),
                    packager: "John Doe <john.doe@example.com>".to_string(),
                },
            )
            .unwrap();

        // TODO: the current behavior is incorrect
        assert_eq!(
            groups.into_vec(),
            &[
                SourcePackageReport {
                    name: "perl".to_string(),
                    version: "5.42.2-1".to_string(),
                    url: "https://mirrors.kernel.org/archlinux/core-testing/os/x86_64/perl-5.42.3-1-x86_64.pkg.tar.zst".to_string(),
                    artifacts: vec![
                            BinaryPackageReport {
                            name: "perl".to_string(),
                            version: "5.42.2-1".to_string(),
                            component: Some("core".to_string()),
                            architecture: "x86_64".to_string(),
                            url: "https://mirrors.kernel.org/archlinux/core/os/x86_64/perl-5.42.2-1-x86_64.pkg.tar.zst".to_string(),
                        },
                        BinaryPackageReport {
                            name: "perl".to_string(),
                            version: "5.42.3-1".to_string(),
                            component: Some("core-testing".to_string()),
                            architecture: "x86_64".to_string(),
                            url: "https://mirrors.kernel.org/archlinux/core-testing/os/x86_64/perl-5.42.3-1-x86_64.pkg.tar.zst".to_string(),
                        },
                    ],
                }
            ]
        );
    }
}
