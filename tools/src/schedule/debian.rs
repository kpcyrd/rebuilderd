use crate::args::PkgsSync;
use crate::schedule::{Pkg, fetch_url_or_path};
use lzma::LzmaReader;
use rebuilderd_common::{PkgGroup, PkgArtifact, Distro};
use rebuilderd_common::errors::*;
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::BufReader;
use std::io::prelude::*;

// TODO: support more archs
pub fn any_architectures() -> Vec<String> {
    vec![
        String::from("amd64"),
    ]
}

#[derive(Debug)]
pub struct DebianPkg {
    base: String,
    binary: Vec<String>,
    version: String,
    directory: String,
    architecture: String,
    uploaders: Vec<String>,
}

impl DebianPkg {
    // this is necessary because the buildinfo folder structure doesn't align with `Directory:` in Sources.xz
    fn buildinfo_path(&self) -> Result<String> {
        let idx = self.directory.find('/') .unwrap();
        let (_, directory) = self.directory.split_at(idx+1);

        let idx = directory.find('/') .unwrap();
        let (_, directory) = directory.split_at(idx+1);

        Ok(directory.to_string())
    }
}

impl Pkg for DebianPkg {
    fn pkg_name(&self) -> &str {
        &self.base
    }

    fn from_maintainer(&self, maintainers: &[String]) -> bool {
        self.uploaders.iter()
            .any(|uploader| maintainers.iter()
                .any(|m| uploader.starts_with(m)))
    }
}

#[derive(Debug, Default)]
pub struct NewPkg {
    base: Option<String>,
    binary: Option<Vec<String>>,
    version: Option<String>,
    directory: Option<String>,
    architecture: Option<String>,
    uploaders: Vec<String>,
    // skip everything that has this set
    extra_source_only: bool,
}

impl TryInto<DebianPkg> for NewPkg {
    type Error = Error;

    fn try_into(self: NewPkg) -> Result<DebianPkg> {
        Ok(DebianPkg {
            base: self.base.ok_or_else(|| format_err!("Missing pkg base field"))?,
            binary: self.binary.ok_or_else(|| format_err!("Missing binary field"))?,
            version: self.version.ok_or_else(|| format_err!("Missing version field"))?,
            directory: self.directory.ok_or_else(|| format_err!("Missing directory field"))?,
            architecture: self.architecture.ok_or_else(|| format_err!("Missing architecture field"))?,
            uploaders: self.uploaders,
        })
    }
}

pub fn extract_pkgs(bytes: &[u8]) -> Result<Vec<DebianPkg>> {
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
                "Package" => pkg.base = Some(b[2..].to_string()),
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

pub fn sync(sync: &PkgsSync) -> Result<Vec<PkgGroup>> {
    let client = reqwest::blocking::Client::new();

    let mut bases: HashMap<_, PkgGroup> = HashMap::new();
    for release in &sync.releases {
        // source looks like: `http://deb.debian.org/debian`
        // should be transformed to eg: `http://deb.debian.org/debian/dists/sid/main/source/Sources.xz`
        let db_url = format!("{}/dists/{}/{}/source/Sources.xz", sync.source, release, sync.suite);
        let bytes = fetch_url_or_path(&client, &db_url)?;

        info!("Decompressing...");
        for pkg in extract_pkgs(&bytes)? {
            if !pkg.matches(&sync) {
                continue;
            }

            let directory = pkg.buildinfo_path()?;
            for arch in expand_architectures(&pkg.architecture)? {
                let url = format!("https://buildinfos.debian.net/buildinfo-pool/{}/{}_{}_{}.buildinfo",
                    directory,
                    pkg.base,
                    pkg.version,
                    arch);

                let mut group = PkgGroup::new(
                    pkg.base.clone(),
                    pkg.version.clone(),
                    Distro::Debian,
                    sync.suite.to_string(),
                    arch.clone(),
                    Some(url),
                );
                for bin in &pkg.binary {
                    group.add_artifact(PkgArtifact {
                        name: bin.to_string(),
                        url: format!("{}/{}/{}_{}_{}.deb",
                            sync.source,
                            pkg.directory,
                            bin,
                            pkg.version,
                            arch,
                        ),
                    });
                }
                bases.insert(format!("{}-{}", pkg.base, pkg.version), group);
            }
        }
    }

    Ok(bases.drain().map(|(_, v)| v).collect())
}
