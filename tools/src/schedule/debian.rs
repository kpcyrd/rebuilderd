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

#[derive(Debug, PartialEq)]
pub enum VersionConstraint {
    Explicit(String),
    Implicit(String),
}

#[derive(Debug, PartialEq)]
pub struct DebianSourcePkg {
    pub base: String,
    pub binary: Vec<String>,
    pub version: String,
    pub directory: String,
    pub architecture: String,
    pub uploaders: Vec<String>,
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
        let version_without_epoch = if let Some((_epoch, version)) = self.version.split_once(':') {
            version
        } else {
            &self.version
        };
        let buildinfo_url = format!("https://buildinfos.debian.net/buildinfo-pool/{}/{}_{}_{}.buildinfo",
            directory,
            self.base,
            version_without_epoch,
            arch);
        buildinfo_url
    }
}

#[derive(Debug, PartialEq)]
pub struct DebianBinPkg {
    name: String,
    version: String,
    source: (String, VersionConstraint),
    architecture: String,
    filename: String,
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
    filename: Option<String>,
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
            filename: new.filename.ok_or_else(|| format_err!("Missing filename field"))?,
            uploaders: new.uploaders,
        })
    }
}

pub fn extract_pkg<T: AnyhowTryFrom<NewPkg>>(bytes: &[u8]) -> Result<Vec<T>> {
    let r = LzmaReader::new_decompressor(bytes)?;
    let r = BufReader::new(r);
    extract_pkgs_uncompressed(r)
}

pub fn extract_pkgs_uncompressed<T: AnyhowTryFrom<NewPkg>, R: BufRead>(r: R) -> Result<Vec<T>> {
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
                "Filename" => pkg.filename = Some(b.to_string()),
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

#[derive(Debug, Default, PartialEq)]
pub struct SyncState {
    groups: HashMap<String, Vec<PkgGroup>>,
}

impl SyncState {
    pub fn new() -> SyncState {
        SyncState::default()
    }

    fn ensure_group_exists(&mut self, src: &DebianSourcePkg, distro: String, suite: String, arch: &str) {
        // TODO: creating a new group isn't always needed
        let buildinfo_url = src.buildinfo_url(arch);
        let new_group = PkgGroup::new(
            src.base.clone(),
            src.version.clone(),
            distro,
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

    fn get_mut_group(&mut self, src: &DebianSourcePkg, distro: String, suite: String, arch: &str) -> &mut PkgGroup {
        self.ensure_group_exists(src, distro, suite, arch);

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

    pub fn push(&mut self, src: &DebianSourcePkg, bin: DebianBinPkg, source: &str, distro: String, suite: String) {
        let group = self.get_mut_group(src, distro, suite, &bin.architecture);
        let url = format!("{}/{}", source, bin.filename);
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
        out.sort_by(|a, b| a.name.cmp(&b.name)
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

                out.push(src, pkg, &sync.source, sync.distro.clone(), sync.suite.clone());
            }
        }
    }

    Ok(out.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

#[test]
    fn test_parse_bin_pkg_simple() {
        let bytes= b"Package: sniffglue
Source: rust-sniffglue
Version: 0.14.0-2
Installed-Size: 2344
Maintainer: Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>
Architecture: amd64
Depends: libc6 (>= 2.32), libgcc-s1 (>= 4.2), libpcap0.8 (>= 1.5.1), libseccomp2 (>= 0.0.0~20120605)
Description: Secure multithreaded packet sniffer
Multi-Arch: allowed
Built-Using: rust-nix (= 0.23.0-1), rust-pktparse (= 0.5.0-1), rust-seccomp-sys (= 0.1.3-1), rustc (= 1.56.0+dfsg1-2)
Description-md5: e7f1183e49341488d3bd8fbe63b63f37
X-Cargo-Built-Using: rust-aho-corasick (= 0.7.10-1), rust-ansi-term (= 0.12.1-1), rust-anyhow (= 1.0.44-2), rust-arrayvec (= 0.5.1-1), rust-atty (= 0.2.14-2), rust-base64 (= 0.13.0-1), rust-bitflags (= 1.2.1-1), rust-block-buffer (= 0.9.0-4), rust-block-padding (= 0.2.1-1), rust-bstr (= 0.2.17-1), rust-byteorder (= 1.4.3-2), rust-cfg-if-0.1 (= 0.1.10-2), rust-cfg-if (= 1.0.0-1), rust-clap (= 2.33.3-1), rust-cpuid-bool (= 0.1.2-4), rust-dhcp4r (= 0.2.0-1), rust-digest (= 0.9.0-1), rust-dirs-next (= 2.0.0-1), rust-dirs-sys-next (= 0.1.1-1), rust-dns-parser (= 0.8.0-1), rust-enum-primitive (= 0.1.1-1), rust-env-logger (= 0.9.0-1), rust-generic-array (= 0.14.4-1), rust-humantime (= 2.1.0-1), rust-itoa (= 0.4.3-1), rust-lazy-static (= 1.4.0-1), rust-lexical-core (= 0.4.8-3), rust-libc (= 0.2.103-1), rust-log (= 0.4.11-2), rust-memchr (= 2.4.1-1), rust-memoffset (= 0.6.4-1), rust-nix (= 0.23.0-1), rust-nom (= 5.0.1-4), rust-num-cpus (= 1.13.0-1), rust-num-traits (= 0.2.14-1), rust-opaque-debug (= 0.3.0-1), rust-pcap-sys (= 0.1.3-2), rust-phf (= 0.8.0-2), rust-phf-shared (= 0.8.0-1), rust-pktparse (= 0.5.0-1), rust-quick-error (= 1.2.3-1), rust-reduce (= 0.1.1-1), rust-regex-automata (= 0.1.8-2), rust-regex (= 1.5.4-1), rust-regex-syntax (= 0.6.25-1), rust-rusticata-macros (= 2.0.4-1), rust-ryu (= 1.0.2-1), rust-seccomp-sys (= 0.1.3-1), rust-serde (= 1.0.130-2), rust-serde-json (= 1.0.41-1), rust-sha2 (= 0.9.2-2), rust-siphasher (= 0.3.1-1), rust-static-assertions (= 1.1.0-1), rust-strsim (= 0.9.3-1), rust-structopt (= 0.3.20-1), rust-strum (= 0.19.2-1), rust-syscallz (= 0.15.0-1), rust-termcolor (= 1.1.0-1), rust-textwrap (= 0.11.0-1), rust-time (= 0.1.42-1), rust-tls-parser (= 0.9.2-3), rust-toml (= 0.5.8-1), rust-typenum (= 1.12.0-1), rust-unicode-width (= 0.1.8-1), rust-users (= 0.11.0-1), rust-vec-map (= 0.8.1-2), rustc (= 1.56.0+dfsg1-2) 
Section: net
Priority: optional
Filename: pool/main/r/rust-sniffglue/sniffglue_0.14.0-2_amd64.deb
Size: 732980
MD5sum: 177f9229266ad5eef3fb42fff0c07345
SHA256: 448c781a9e594227bc9f0d6c65b8beba2b3add68d3583020de188d4cfa365b40

";
        let cursor = Cursor::new(bytes);
        let pkgs = extract_pkgs_uncompressed::<DebianBinPkg, _>(cursor).unwrap();
        assert_eq!(&pkgs, &[
            DebianBinPkg {
                name: "sniffglue".to_string(),
                version: "0.14.0-2".to_string(),
                source: ("rust-sniffglue".to_string(), VersionConstraint::Implicit("0.14.0-2".to_string())),
                architecture: "amd64".to_string(),
                filename: "pool/main/r/rust-sniffglue/sniffglue_0.14.0-2_amd64.deb".to_string(),
                uploaders: vec![],
            },
        ]);
    }

    #[test]
    fn test_parse_bin_pkg_with_epoch() {
        let bytes = b"Package: mariadb-server
Source: mariadb-10.5
Version: 1:10.5.12-1
Installed-Size: 71
Maintainer: Debian MySQL Maintainers <pkg-mysql-maint@lists.alioth.debian.org>
Architecture: all
Depends: mariadb-server-10.5 (>= 1:10.5.12-1)
Description: MariaDB database server (metapackage depending on the latest version)
Homepage: https://mariadb.org/
Description-md5: a887647d25d472f89e220ceda2b6e041
Tag: devel::lang:c++, devel::lang:sql, devel::library, implemented-in::c++,
 interface::commandline, interface::daemon, network::server,
 protocol::db:mysql, role::devel-lib, role::metapackage, role::program,
 works-with::db
Section: database
Priority: optional
Filename: pool/main/m/mariadb-10.5/mariadb-server_10.5.12-1_all.deb
Size: 34856
MD5sum: 0dc2d34ec8673612de925bb516cadc51
SHA256: 0db2ae9db7de7cd88b02741a3fb19cf66f0043d56dcb129d91578f269973b286

";
        let cursor = Cursor::new(bytes);
        let pkgs = extract_pkgs_uncompressed::<DebianBinPkg, _>(cursor).unwrap();
        assert_eq!(&pkgs, &[
            DebianBinPkg {
                name: "mariadb-server".to_string(),
                version: "1:10.5.12-1".to_string(),
                source: ("mariadb-10.5".to_string(), VersionConstraint::Implicit("1:10.5.12-1".to_string())),
                architecture: "all".to_string(),
                filename: "pool/main/m/mariadb-10.5/mariadb-server_10.5.12-1_all.deb".to_string(),
                uploaders: vec![],
            },
        ]);
    }

    #[test]
    fn test_parse_source_pkg() {
        let bytes = "Package: mariadb-10.5
Binary: libmariadb-dev, libmariadb-dev-compat, libmariadb3, libmariadbd19, libmariadbd-dev, mariadb-common, mariadb-client-core-10.5, mariadb-client-10.5, mariadb-server-core-10.5, mariadb-server-10.5, mariadb-server, mariadb-client, mariadb-backup, mariadb-plugin-connect, mariadb-plugin-s3, mariadb-plugin-rocksdb, mariadb-plugin-oqgraph, mariadb-plugin-mroonga, mariadb-plugin-spider, mariadb-plugin-gssapi-server, mariadb-plugin-gssapi-client, mariadb-plugin-cracklib-password-check, mariadb-test, mariadb-test-data
Version: 1:10.5.12-1
Maintainer: Debian MySQL Maintainers <pkg-mysql-maint@lists.alioth.debian.org>
Uploaders: Otto Kekäläinen <otto@debian.org>
Build-Depends: bison, cmake, cracklib-runtime <!nocheck>, debhelper (>= 10), dh-exec, gdb <!nocheck>, libaio-dev [linux-any], libboost-dev, libcrack2-dev (>= 2.9.0), libcurl4-openssl-dev | libcurl4-dev, libedit-dev, libedit-dev:native, libjemalloc-dev [linux-any], libjudy-dev, libkrb5-dev, liblz4-dev, libncurses5-dev (>= 5.0-6~), libncurses5-dev:native (>= 5.0-6~), libnuma-dev [linux-any], libpam0g-dev, libpcre2-dev, libsnappy-dev, libssl-dev, libssl-dev:native, libsystemd-dev [linux-any], libxml2-dev, libzstd-dev (>= 1.3.3), lsb-release, perl:any, po-debconf, psmisc, unixodbc-dev, uuid-dev, zlib1g-dev (>= 1:1.1.3-5~)
Architecture: any all
Standards-Version: 4.5.0
Format: 3.0 (quilt)
Files:
 a0aa3e2839d3caf1b17a653968bd315d 4782 mariadb-10.5_10.5.12-1.dsc
 2b103e8a462fd1a5ce6fc9dec4c23844 101914615 mariadb-10.5_10.5.12.orig.tar.gz
 5c8baca3552e0a2b6400588fba1088d4 195 mariadb-10.5_10.5.12.orig.tar.gz.asc
 4e7509cb525aa7a8be9ac405ef53d784 222380 mariadb-10.5_10.5.12-1.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/mariadb-team/mariadb-10.5
Vcs-Git: https://salsa.debian.org/mariadb-team/mariadb-10.5.git
Checksums-Sha256:
 0092ed302d547dd00843ada1bc85a783ee0a7aa4f0448b91d744226c6b311d55 4782 mariadb-10.5_10.5.12-1.dsc
 ab4f1ca69a30c5372e191a68e8b543a74168327680fb1f4067e8cc0a5582e4bd 101914615 mariadb-10.5_10.5.12.orig.tar.gz
 0d3b97cfb7998fd0a62cbc6e6d8ae1340d52b616ac12e9eb8f2773a9775f82c6 195 mariadb-10.5_10.5.12.orig.tar.gz.asc
 451dbbdaaedb6cf087ba38dd2708de6a31fa88a5333295be90c2323a414dc53c 222380 mariadb-10.5_10.5.12-1.debian.tar.xz
Homepage: https://mariadb.org/
Package-List: 
 libmariadb-dev deb libdevel optional arch=any
 libmariadb-dev-compat deb libdevel optional arch=any
 libmariadb3 deb libs optional arch=any
 libmariadbd-dev deb libdevel optional arch=any
 libmariadbd19 deb libs optional arch=any
 mariadb-backup deb database optional arch=any
 mariadb-client deb database optional arch=all
 mariadb-client-10.5 deb database optional arch=any
 mariadb-client-core-10.5 deb database optional arch=any
 mariadb-common deb database optional arch=all
 mariadb-plugin-connect deb database optional arch=any
 mariadb-plugin-cracklib-password-check deb database optional arch=any
 mariadb-plugin-gssapi-client deb database optional arch=any
 mariadb-plugin-gssapi-server deb database optional arch=any
 mariadb-plugin-mroonga deb database optional arch=any-alpha,any-amd64,any-arm,any-arm64,any-i386,any-ia64,any-mips64el,any-mips64r6el,any-mipsel,any-mipsr6el,any-nios2,any-powerpcel,any-ppc64el,any-sh3,any-sh4,any-tilegx
 mariadb-plugin-oqgraph deb database optional arch=any
 mariadb-plugin-rocksdb deb database optional arch=amd64,arm64,mips64el,ppc64el
 mariadb-plugin-s3 deb database optional arch=any
 mariadb-plugin-spider deb database optional arch=any
 mariadb-server deb database optional arch=all
 mariadb-server-10.5 deb database optional arch=any
 mariadb-server-core-10.5 deb database optional arch=any
 mariadb-test deb database optional arch=any
 mariadb-test-data deb database optional arch=all
Testsuite: autopkgtest
Testsuite-Triggers: eatmydata
Directory: pool/main/m/mariadb-10.5
Priority: extra
Section: misc

";
        let cursor = Cursor::new(bytes);
        let pkgs = extract_pkgs_uncompressed::<DebianSourcePkg, _>(cursor).unwrap();
        assert_eq!(&pkgs, &[
            DebianSourcePkg {
                base: "mariadb-10.5".to_string(),
                binary: vec![
                    "libmariadb-dev".to_string(),
                    "libmariadb-dev-compat".to_string(),
                    "libmariadb3".to_string(),
                    "libmariadbd19".to_string(),
                    "libmariadbd-dev".to_string(),
                    "mariadb-common".to_string(),
                    "mariadb-client-core-10.5".to_string(),
                    "mariadb-client-10.5".to_string(),
                    "mariadb-server-core-10.5".to_string(),
                    "mariadb-server-10.5".to_string(),
                    "mariadb-server".to_string(),
                    "mariadb-client".to_string(),
                    "mariadb-backup".to_string(),
                    "mariadb-plugin-connect".to_string(),
                    "mariadb-plugin-s3".to_string(),
                    "mariadb-plugin-rocksdb".to_string(),
                    "mariadb-plugin-oqgraph".to_string(),
                    "mariadb-plugin-mroonga".to_string(),
                    "mariadb-plugin-spider".to_string(),
                    "mariadb-plugin-gssapi-server".to_string(),
                    "mariadb-plugin-gssapi-client".to_string(),
                    "mariadb-plugin-cracklib-password-check".to_string(),
                    "mariadb-test".to_string(),
                    "mariadb-test-data".to_string(),
                ],
                version: "1:10.5.12-1".to_string(),
                directory: "pool/main/m/mariadb-10.5".to_string(),
                architecture: "any all".to_string(),
                uploaders: vec!["Otto Kekäläinen <otto@debian.org>".to_string()],
            }
        ]);
    }

    #[test]
    fn test_generate_group() {
        let src = DebianSourcePkg {
            base: "mariadb-10.5".to_string(),
            binary: vec![
                "libmariadb-dev".to_string(),
                "libmariadb-dev-compat".to_string(),
                "libmariadb3".to_string(),
                "libmariadbd19".to_string(),
                "libmariadbd-dev".to_string(),
                "mariadb-common".to_string(),
                "mariadb-client-core-10.5".to_string(),
                "mariadb-client-10.5".to_string(),
                "mariadb-server-core-10.5".to_string(),
                "mariadb-server-10.5".to_string(),
                "mariadb-server".to_string(),
                "mariadb-client".to_string(),
                "mariadb-backup".to_string(),
                "mariadb-plugin-connect".to_string(),
                "mariadb-plugin-s3".to_string(),
                "mariadb-plugin-rocksdb".to_string(),
                "mariadb-plugin-oqgraph".to_string(),
                "mariadb-plugin-mroonga".to_string(),
                "mariadb-plugin-spider".to_string(),
                "mariadb-plugin-gssapi-server".to_string(),
                "mariadb-plugin-gssapi-client".to_string(),
                "mariadb-plugin-cracklib-password-check".to_string(),
                "mariadb-test".to_string(),
                "mariadb-test-data".to_string(),
            ],
            version: "1:10.5.12-1".to_string(),
            directory: "pool/main/m/mariadb-10.5".to_string(),
            architecture: "any all".to_string(),
            uploaders: vec!["Otto Kekäläinen <otto@debian.org>".to_string()],
        };
        let bin = DebianBinPkg {
            name: "mariadb-server".to_string(),
            version: "1:10.5.12-1".to_string(),
            source: ("mariadb-10.5".to_string(), VersionConstraint::Implicit("1:10.5.12-1".to_string())),
            architecture: "all".to_string(),
            filename: "pool/main/m/mariadb-10.5/mariadb-server_10.5.12-1_all.deb".to_string(),
            uploaders: vec![],
        };
        let mut state = SyncState::new();
        state.push(&src, bin, "https://deb.debian.org/debian", "debian".to_string(), "main".to_string());

        let mut groups = HashMap::new();
        groups.insert("mariadb-10.5".to_string(), vec![
            PkgGroup {
                name: "mariadb-10.5".to_string(),
                version: "1:10.5.12-1".to_string(),
                distro: "debian".to_string(),
                suite: "main".to_string(),
                architecture: "all".to_string(),
                input_url: Some("https://buildinfos.debian.net/buildinfo-pool/m/mariadb-10.5/mariadb-10.5_10.5.12-1_all.buildinfo".to_string()),
                artifacts: vec![
                    PkgArtifact {
                        name: "mariadb-server".to_string(),
                        version: "1:10.5.12-1".to_string(),
                        url: "https://deb.debian.org/debian/pool/main/m/mariadb-10.5/mariadb-server_10.5.12-1_all.deb".to_string(),
                    }
                ],
            },
        ]);

        assert_eq!(state, SyncState {
            groups,
        });
    }
}
