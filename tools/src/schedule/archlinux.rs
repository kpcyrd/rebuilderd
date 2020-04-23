use crate::schedule::{Pkg, url_or_path};
use crate::PkgsSync;
use flate2::read::GzDecoder;
use rebuilderd_common::{PkgRelease, Distro, Status};
use rebuilderd_common::errors::*;
use std::convert::TryInto;
use std::io::prelude::*;
use tar::{Archive, EntryType};

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
    let client = reqwest::blocking::Client::new();
    let bytes = url_or_path(&client, &sync.source)?;

    info!("Parsing index...");
    let mut pkgs = Vec::new();
    for pkg in extract_pkgs(&bytes)? {
        if !pkg.matches(&sync) {
            continue;
        }

        let url = format!("https://mirrors.kernel.org/archlinux/{}/os/{}/{}",
            sync.suite,
            sync.architecture,
            pkg.filename);
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
