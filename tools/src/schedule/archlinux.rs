use crate::args::PkgsSync;
use crate::decompress;
use crate::schedule::{fetch_url_or_path, Pkg};
use nom::bytes::complete::take_till;
use rebuilderd_common::api::v1::{BinaryPackageReport, PackageReport, SourcePackageReport};
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::prelude::*;
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

#[derive(Debug)]
pub struct ArchPkg {
    pub name: String,
    pub base: String,
    pub filename: String,
    pub version: String,
    pub architecture: String,
    pub packager: String,
}

impl Pkg for ArchPkg {
    fn pkg_name(&self) -> &str {
        &self.name
    }

    fn by_maintainer(&self, maintainers: &[String]) -> bool {
        maintainers.iter().any(|m| self.packager.starts_with(m))
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

pub async fn sync(http: &http::Client, sync: &PkgsSync) -> Result<Vec<PackageReport>> {
    let source = if sync.source.ends_with(".db") {
        warn!("Detected legacy configuration for source, use the new format instead: https://mirrors.kernel.org/archlinux/$repo/os/$arch");
        "https://mirrors.kernel.org/archlinux/$repo/os/$arch"
    } else {
        &sync.source
    };

    let mut reports = Vec::new();
    for arch in &sync.architectures {
        let db = mirror_to_url(source, &sync.suite, arch, &format!("{}.db", sync.suite))?;
        let bytes = fetch_url_or_path(http, &db).await?;

        let mut report = PackageReport {
            distribution: "archlinux".to_string(),
            release: None,
            component: Some(sync.suite.clone()),
            architecture: arch.clone(),
            packages: Vec::new(),
        };

        let mut bases: HashMap<_, SourcePackageReport> = HashMap::new();

        info!("Parsing index ({} bytes)...", bytes.len());
        for pkg in extract_pkgs(&bytes)? {
            if !pkg.matches(sync) {
                continue;
            }

            let url = mirror_to_url(source, &sync.suite, arch, &pkg.filename)?;
            let artifact = BinaryPackageReport {
                name: pkg.name,
                version: pkg.version.clone(),
                architecture: pkg.architecture,
                url: url.clone(),
            };

            if let Some(group) = bases.get_mut(&pkg.base) {
                // TODO: multiple architectures could have the exact same package with arch=any
                group.artifacts.push(artifact);
            } else {
                let mut group = SourcePackageReport {
                    name: pkg.base.clone(),
                    version: pkg.version.clone(),
                    url: url.clone(), // use first artifact's url as the source URL for now
                    artifacts: Vec::new(),
                };

                group.artifacts.push(artifact);
                bases.insert(pkg.base, group);
            }
        }

        report.packages = bases.into_values().collect();
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
}
