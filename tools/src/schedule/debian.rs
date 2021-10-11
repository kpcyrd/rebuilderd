use crate::args::PkgsSync;
use crate::schedule::{Pkg, fetch_url_or_path};
use lzma::LzmaReader;
use rebuilderd_common::{PkgGroup, PkgArtifact};
use rebuilderd_common::errors::*;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::prelude::*;

// TODO: support more archs
pub fn any_architectures() -> Vec<String> {
    vec![
        String::from("amd64"),
    ]
}

#[derive(Debug, Default)]
pub struct SourcePkgBucket {
    pkgs: HashMap<String, Vec<DebianSourcePkg>>,
}

impl SourcePkgBucket {
    pub fn new() -> SourcePkgBucket {
        SourcePkgBucket::default()
    }

    pub fn push(&mut self, pkg: DebianSourcePkg) {
        if let Some(list) = self.pkgs.get_mut(&pkg.base) {
            list.push(pkg);
        } else {
            self.pkgs.insert(pkg.base.clone(), vec![pkg]);
        }
    }

    pub fn get(&self, pkg: &DebianBinPkg) -> Result<&DebianSourcePkg> {
        let (name, version) = &pkg.source;
        let list = self.pkgs.get(name)
            .with_context(|| anyhow!("No source package found with name: {:?}", name))?;

        // we currently track if the version was set explicitly or implicitly, keeping track just in case
        let version = match version {
            VersionConstraint::Explicit(version) => version,
            VersionConstraint::Implicit(version) => version,
        };

        for src in list {
            if src.version == *version {
                return Ok(src);
            }
        }

        bail!("No matching source package found")
    }
}

#[derive(Debug)]
pub enum VersionConstraint {
    Explicit(String),
    Implicit(String),
}

#[derive(Debug)]
pub struct DebianSourcePkg {
    base: String,
    binary: Vec<String>,
    version: String,
    directory: String,
    architecture: String,
    uploaders: Vec<String>,
}

impl DebianSourcePkg {
    // this is necessary because the buildinfo folder structure doesn't align with `Directory:` in Sources.xz
    fn buildinfo_path(&self) -> String {
        let idx = self.directory.find('/') .unwrap();
        let (_, directory) = self.directory.split_at(idx+1);

        let idx = directory.find('/') .unwrap();
        let (_, directory) = directory.split_at(idx+1);

        directory.to_string()
    }

    fn buildinfo_url(&self, arch: &str) -> String {
        let directory = self.buildinfo_path();
        let buildinfo_url = format!("https://buildinfos.debian.net/buildinfo-pool/{}/{}_{}_{}.buildinfo",
            directory,
            self.base,
            self.version,
            arch);
        buildinfo_url
    }
}

#[derive(Debug)]
pub struct DebianBinPkg {
    name: String,
    version: String,
    source: (String, VersionConstraint),
    architecture: String,
    uploaders: Vec<String>,
}

impl Pkg for DebianBinPkg {
    fn pkg_name(&self) -> &str {
        &self.name
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
    source: Option<String>,
    version: Option<String>,
    directory: Option<String>,
    architecture: Option<String>,
    uploaders: Vec<String>,
    // skip everything that has this set
    extra_source_only: bool,
}

pub trait AnyhowTryFrom<T>: Sized {
    fn try_from(x: T) -> Result<Self>;
}

impl AnyhowTryFrom<NewPkg> for DebianSourcePkg {
    fn try_from(new: NewPkg) -> Result<Self> {
        Ok(DebianSourcePkg {
            base: new.base.ok_or_else(|| format_err!("Missing pkg base field"))?,
            binary: new.binary.ok_or_else(|| format_err!("Missing binary field"))?,
            version: new.version.ok_or_else(|| format_err!("Missing version field"))?,
            directory: new.directory.ok_or_else(|| format_err!("Missing directory field"))?,
            architecture: new.architecture.ok_or_else(|| format_err!("Missing architecture field"))?,
            uploaders: new.uploaders,
        })
    }
}

impl AnyhowTryFrom<NewPkg> for DebianBinPkg {
    fn try_from(new: NewPkg) -> Result<Self> {
        let name = new.base.ok_or_else(|| format_err!("Missing pkg base field"))?;
        let version = new.version.ok_or_else(|| format_err!("Missing version field"))?;

        let source = if let Some(source) = new.source {
            if let Some((name, version)) = source.split_once(' ') {
                let version = version.strip_prefix('(')
                    .context("Malformed version in Source:")?
                    .strip_suffix(')')
                    .context("Malformed version in Source:")?
                    .to_string();
                (name.to_string(), VersionConstraint::Explicit(version))
            } else {
                (source, VersionConstraint::Implicit(version.clone()))
            }
        } else {
            (name.clone(), VersionConstraint::Implicit(version.clone()))
        };

        Ok(DebianBinPkg {
            name,
            version,
            source,
            architecture: new.architecture.ok_or_else(|| format_err!("Missing architecture field"))?,
            uploaders: new.uploaders,
        })
    }
}

pub fn extract_pkg<T: AnyhowTryFrom<NewPkg>>(bytes: &[u8]) -> Result<Vec<T>> {
    let r = LzmaReader::new_decompressor(bytes)?;
    let r = BufReader::new(r);

    let mut pkg = NewPkg::default();
    let mut pkgs = Vec::new();
    let mut lines = r.lines().peekable();

    while let Some(line) = lines.next() {
        let line = line?;
        if line.is_empty() {
            if !pkg.extra_source_only {
                pkgs.push(T::try_from(pkg)?);
            }
            pkg = NewPkg::default();
        }
        if let Some((a, b)) = line.split_once(": ") {
            match a {
                "Package" => pkg.base = Some(b.to_string()),
                "Binary" => {
                    let mut binaries = Vec::new();

                    let mut value = b.to_string();
                    while value.ends_with(',') {
                        if let Some(line) = lines.peek() {
                            if let Ok(line) = line {
                                if !line.starts_with(' ') {
                                    warn!("Line ended with comma, but next line is not multi-line");
                                    break;
                                }
                            }
                        } else {
                            break;
                        }

                        let line = lines.next()
                            .ok_or_else(|| anyhow!("Found comma on last line of file"))?
                            .context("Failed to read line")?;
                        value.push_str(&line);
                    }

                    for binary in value.split(", ") {
                        binaries.push(binary.to_string());
                    }
                    pkg.binary = Some(binaries);
                },
                "Version" => pkg.version = Some(b.to_string()),
                "Source" => pkg.source = Some(b.to_string()),
                "Directory" => pkg.directory = Some(b.to_string()),
                "Architecture" => pkg.architecture = Some(b.to_string()),
                "Uploaders" => {
                    let mut uploaders = Vec::new();
                    for uploader in b.split(", ") {
                        uploaders.push(uploader.to_string());
                    }
                    pkg.uploaders = uploaders;
                },
                "Extra-Source-Only" => if b == "yes" {
                    pkg.extra_source_only = true;
                },
                _ => (),
            }
        }
    }

    Ok(pkgs)
}

#[derive(Debug, Default)]
pub struct SyncState {
    groups: HashMap<String, Vec<PkgGroup>>,
}

impl SyncState {
    pub fn new() -> SyncState {
        SyncState::default()
    }

    fn ensure_group_exists(&mut self, src: &DebianSourcePkg, suite: String, arch: &str) {
        // TODO: creating a new group isn't always needed
        let buildinfo_url = src.buildinfo_url(arch);
        let new_group = PkgGroup::new(
            src.base.clone(),
            src.version.clone(),
            "debian".to_string(),
            suite,
            arch.to_string(),
            Some(buildinfo_url),
        );

        if let Some(list) = self.groups.get_mut(&src.base) {
            for group in list.iter() {
                if group.version == src.version && group.architecture == arch {
                    return;
                }
            }

            list.push(new_group);
        } else {
            self.groups.insert(src.base.clone(), vec![new_group]);
        }
    }

    fn get_mut_group(&mut self, src: &DebianSourcePkg, suite: String, arch: &str) -> &mut PkgGroup {
        self.ensure_group_exists(src, suite, arch);

        // ensure_group_exists ensures the group exists
        let list = self.groups.get_mut(&src.base).unwrap();

        for group in list {
            if group.version == src.version && group.architecture == arch {
                return group;
            }
        }

        // ensure_group_exists ensures the group exists
        unreachable!()
    }

    pub fn push(&mut self, src: &DebianSourcePkg, bin: DebianBinPkg, source: &str, suite: String) {
        let group = self.get_mut_group(src, suite, &bin.architecture);
        let url = format!("{}/{}/{}_{}_{}.deb",
            source,
            src.directory,
            bin.name,
            bin.version,
            bin.architecture,
        );
        group.add_artifact(PkgArtifact {
            name: bin.name,
            version: bin.version,
            url,
        });
    }

    pub fn to_vec(self) -> Vec<PkgGroup> {
        let mut out = self.groups.into_values()
            .flatten()
            .collect::<Vec<_>>();
        out.sort_by(|a, b| a.base.cmp(&b.base)
            .then_with(|| a.version.cmp(&b.version)));
        out
    }
}

pub async fn sync(sync: &PkgsSync) -> Result<Vec<PkgGroup>> {
    let client = reqwest::Client::new();

    if sync.releases.len() > 1 {
        bail!("Tracking multiple releases in the same rebuilder is currently unsupported");
    }

    let mut out = SyncState::new();

    for release in &sync.releases {
        let mut sources = SourcePkgBucket::new();

        // Downloading source package index
        let db_url = format!("{}/dists/{}/{}/source/Sources.xz", sync.source, release, sync.suite);

        let bytes = fetch_url_or_path(&client, &db_url)
            .await?;

        info!("Building map of all source packages");
        for pkg in extract_pkg::<DebianSourcePkg>(&bytes)? {
            debug!("Registering pkgbase: {:?}", pkg.base);
            sources.push(pkg);
        }

        for arch in &sync.architectures {
            // Downloading binary package index
            let db_url = format!("{}/dists/{}/{}/binary-{}/Packages.xz", sync.source, release, sync.suite, arch);

            let bytes = fetch_url_or_path(&client, &db_url)
                .await?;

            for pkg in extract_pkg::<DebianBinPkg>(&bytes)? {
                if !pkg.matches(sync) {
                    continue;
                }

                debug!("Found binary package: {:?} {:?}", pkg.name, pkg.version);

                let src = sources.get(&pkg)?;
                debug!("Matched binary package to source package: {:?} {:?}", src.base, src.version);

                out.push(src, pkg, &sync.source, sync.suite.clone());
            }
        }
    }

    Ok(out.to_vec())
}
