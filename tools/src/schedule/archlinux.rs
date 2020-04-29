use crate::schedule::{Pkg, fetch_url_or_path};
use crate::PkgsSync;
use flate2::read::GzDecoder;
use nom::bytes::complete::take_till;
use rebuilderd_common::{PkgRelease, Distro, Status};
use rebuilderd_common::errors::*;
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
    pub filename: String,
    pub version: String,
    pub architecture: String,
    pub packager: String,
}

impl Pkg for ArchPkg {
    fn pkg_name(&self) -> &str {
        &self.name
    }

    fn from_maintainer(&self, maintainers: &[String]) -> bool {
        maintainers.iter()
            .any(|m| self.packager.starts_with(m))
    }
}

#[derive(Debug, Default)]
pub struct NewPkg {
    name: Vec<String>,
    filename: Vec<String>,
    version: Vec<String>,
    architecture: Vec<String>,
    packager: Vec<String>,
}

impl TryInto<ArchPkg> for NewPkg {
    type Error = Error;

    fn try_into(self: NewPkg) -> Result<ArchPkg> {
        Ok(ArchPkg {
            name: self.name.get(0).ok_or_else(|| format_err!("Missing name field"))?.to_string(),
            filename: self.filename.get(0).ok_or_else(|| format_err!("Missing filename field"))?.to_string(),
            version: self.version.get(0).ok_or_else(|| format_err!("Missing version field"))?.to_string(),
            architecture: self.architecture.get(0).ok_or_else(|| format_err!("Missing architecture field"))?.to_string(),
            packager: self.packager.get(0).ok_or_else(|| format_err!("Missing packager field"))?.to_string(),
        })
    }
}

pub fn extract_pkgs(bytes: &[u8]) -> Result<Vec<ArchPkg>> {
    let tar = GzDecoder::new(&bytes[..]);
    let mut archive = Archive::new(tar);

    let mut pkgs = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.header().entry_type() == EntryType::Regular {
            let mut pkg = NewPkg::default();

            let mut content = String::new();
            entry.read_to_string(&mut content)?;

            let mut iter = content.split('\n');
            loop {
                let key = match iter.next() {
                    Some(key) => key,
                    _ => break,
                };

                let mut values = Vec::new();
                loop {
                    let value = match iter.next() {
                        Some(value) => value,
                        _ => break,
                    };
                    if !value.is_empty() {
                        values.push(value.to_string());
                    } else {
                        break;
                    }
                }

                match key {
                    "%FILENAME%" => pkg.filename = values,
                    "%NAME%" => pkg.name = values,
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

pub fn sync(sync: &PkgsSync) -> Result<Vec<PkgRelease>> {
    let source = if sync.source.ends_with(".db") {
        warn!("Detected legacy configuration for source, use the new format instead: https://mirrors.kernel.org/archlinux/$repo/os/$arch");
        "https://mirrors.kernel.org/archlinux/$repo/os/$arch"
    } else {
        &sync.source
    };

    let client = reqwest::blocking::Client::new();
    let db = mirror_to_url(&source, &sync.suite, &sync.architecture, &format!("{}.db", sync.suite))?;
    let bytes = fetch_url_or_path(&client, &db)?;

    info!("Parsing index...");
    let mut pkgs = Vec::new();
    for pkg in extract_pkgs(&bytes)? {
        if !pkg.matches(&sync) {
            continue;
        }

        let url = mirror_to_url(&source, &sync.suite, &sync.architecture, &pkg.filename)?;
        pkgs.push(PkgRelease {
            name: pkg.name,
            version: pkg.version,
            status: Status::Unknown,
            distro: Distro::Archlinux.to_string(),
            suite: sync.suite.to_string(),
            architecture: pkg.architecture,
            url,
        });
    }

    Ok(pkgs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mirror_to_url() {
        let url = mirror_to_url("https://ftp.halifax.rwth-aachen.de/archlinux/$repo/os/$arch", "core", "x86_64", "core.db").unwrap();
        assert_eq!(url, "https://ftp.halifax.rwth-aachen.de/archlinux/core/os/x86_64/core.db");
    }
}
