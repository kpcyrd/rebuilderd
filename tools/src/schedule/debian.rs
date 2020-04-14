use rebuilderd_common::errors::*;
use rebuilderd_common::Status;
use lzma::LzmaReader;
use std::io::BufReader;
use std::io::prelude::*;
use rebuilderd_common::PkgRelease;
use rebuilderd_common::Distro;
use crate::schedule::url_or_path;
use crate::PkgsSync;

// TODO: support more archs
pub fn any_architectures() -> Vec<String> {
    vec![
        String::from("amd64"),
    ]
}

#[derive(Debug)]
pub struct Pkg {
    package: String,
    binary: Vec<String>,
    version: String,
    directory: String,
    architecture: String,
    uploaders: Vec<String>,
}

impl Pkg {
    fn buildinfo_path(&self) -> Result<String> {
        let idx = self.directory.find('/') .unwrap();
        let (_, directory) = self.directory.split_at(idx+1);

        let idx = directory.find('/') .unwrap();
        let (_, directory) = directory.split_at(idx+1);

        Ok(directory.to_string())
    }

    fn from_maintainer(&self, maintainers: &[String]) -> bool {
        if maintainers.is_empty() {
            true
        } else {
            self.uploaders.iter()
                .any(|uploader| maintainers.iter()
                    .any(|m| uploader.starts_with(m)))
        }
    }
}

#[derive(Debug, Default)]
pub struct NewPkg {
    package: Option<String>,
    binary: Option<Vec<String>>,
    version: Option<String>,
    directory: Option<String>,
    architecture: Option<String>,
    uploaders: Vec<String>,
    // skip everything that has this set
    extra_source_only: bool,
}

use std::convert::TryInto;
impl TryInto<Pkg> for NewPkg {
    type Error = Error;

    fn try_into(self: NewPkg) -> Result<Pkg> {
        Ok(Pkg {
            package: self.package.ok_or_else(|| format_err!("Missing package field"))?,
            binary: self.binary.ok_or_else(|| format_err!("Missing binary field"))?,
            version: self.version.ok_or_else(|| format_err!("Missing version field"))?,
            directory: self.directory.ok_or_else(|| format_err!("Missing directory field"))?,
            architecture: self.architecture.ok_or_else(|| format_err!("Missing architecture field"))?,
            uploaders: self.uploaders,
        })
    }
}

pub fn extract_pkgs(bytes: &[u8]) -> Result<Vec<Pkg>> {
    let r = LzmaReader::new_decompressor(&bytes[..])?;
    let r = BufReader::new(r);

    let mut pkg = NewPkg::default();
    let mut pkgs = Vec::new();
    for line in r.lines() {
        let line = line?;
        if line.is_empty() {
            if !pkg.extra_source_only {
                pkgs.push(pkg.try_into()?);
            }
            pkg = NewPkg::default();
        }
        if let Some(idx) = line.find(": ") {
            let (a, b) = line.split_at(idx);
            match a {
                "Package" => pkg.package = Some(b[2..].to_string()),
                "Binary" => {
                    let mut binaries = Vec::new();
                    for binary in b[2..].split(", ") {
                        binaries.push(binary.to_string());
                    }
                    pkg.binary = Some(binaries);
                },
                "Version" => pkg.version = Some(b[2..].to_string()),
                "Directory" => pkg.directory = Some(b[2..].to_string()),
                "Architecture" => pkg.architecture = Some(b[2..].to_string()),
                "Uploaders" => {
                    let mut uploaders = Vec::new();
                    for uploader in b[2..].split(", ") {
                        uploaders.push(uploader.to_string());
                    }
                    pkg.uploaders = uploaders;
                },
                "Extra-Source-Only" => if &b[2..] == "yes" {
                    pkg.extra_source_only = true;
                },
                _ => (),
            }
        }
    }

    Ok(pkgs)
}

pub fn expand_architectures(arch: &str) -> Result<Vec<String>> {
    match arch {
        "all" => Ok(vec![String::from("all")]),
        "any" => Ok(any_architectures()),
        a => {
            for arch in a.split(' ') {
                if arch == "amd64" {
                    return Ok(vec![arch.to_string()]);
                }
            }
            Ok(vec![])
        },
    }
}

pub fn sync(sync: &PkgsSync) -> Result<Vec<PkgRelease>> {
    let client = reqwest::blocking::Client::new();
    let bytes = url_or_path(&client, &sync.source)?;

    info!("Decompressing...");
    let mut pkgs = Vec::new();
    for pkg in extract_pkgs(&bytes)? {
        if !pkg.from_maintainer(&sync.maintainer) {
            continue;
        }

        let directory = pkg.buildinfo_path()?;
        for bin in &pkg.binary {
            for arch in expand_architectures(&pkg.architecture)? {
                let url = format!("https://buildinfos.debian.net/buildinfo-pool/{}/{}_{}_{}.buildinfo",
                    directory,
                    bin,
                    pkg.version,
                    arch);

                pkgs.push(PkgRelease {
                    name: bin.to_string(),
                    version: pkg.version.to_string(),
                    status: Status::Unknown,
                    distro: Distro::Debian.to_string(),
                    suite: sync.suite.to_string(),
                    architecture: arch,
                    url,
                });
            }
        }
    }

    Ok(pkgs)
}
