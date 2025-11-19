use crate::args::PkgsSync;
use crate::schedule::{Pkg, fetch_url_or_path};
use rebuilderd_common::api::v1::{BinaryPackageReport, PackageReport, SourcePackageReport};
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::prelude::*;
use xz2::read::XzDecoder;

pub const BIN_NMU_PREFIX: &str = "+b";

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

    pub fn get(&self, pkg: &DebianBinPkg) -> Result<DebianSourcePkg> {
        let (name, version) = &pkg.source;
        let bin_nmu = pkg
            .version
            .rfind(BIN_NMU_PREFIX)
            .map(|idx| pkg.version.split_at(idx).1)
            .filter(|num| num[BIN_NMU_PREFIX.len()..].parse::<u64>().is_ok())
            .unwrap_or("");
        let list = self
            .pkgs
            .get(name)
            .with_context(|| anyhow!("No source package found with name: {:?}", name))?;

        // we currently track if the version was set explicitly or implicitly, keeping track just in case
        let version = match version {
            VersionConstraint::Explicit(version) => version,
            VersionConstraint::Implicit(version) => version,
        };

        for src in list {
            if src.version == *version {
                let mut src_cpy = src.clone();
                src_cpy.version.push_str(bin_nmu);
                return Ok(src_cpy);
            }
        }

        bail!("No matching source package found")
    }

    pub fn import_compressed_source_package_file(&mut self, bytes: &[u8]) -> Result<()> {
        for pkg in extract_pkgs_compressed::<DebianSourcePkg>(bytes)? {
            debug!("Registering pkgbase: {:?}", pkg.base);
            self.push(pkg);
        }
        Ok(())
    }

    pub fn import_uncompressed_source_package_file<R: BufRead>(&mut self, reader: R) -> Result<()> {
        for pkg in extract_pkgs_uncompressed::<DebianSourcePkg, R>(reader)? {
            debug!("Registering pkgbase: {:?}", pkg.base);
            self.push(pkg);
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum VersionConstraint {
    Explicit(String),
    Implicit(String),
}

#[derive(Debug, PartialEq, Eq, Clone)]
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
        if self.base.starts_with("lib") {
            self.base[..4].to_string()
        } else {
            self.base[..1].to_string()
        }
    }

    fn buildinfo_url(&self, arch: &str) -> String {
        let directory = self.buildinfo_path();
        let version_without_epoch = if let Some((_epoch, version)) = self.version.split_once(':') {
            version
        } else {
            &self.version
        };
        let buildinfo_url = format!(
            "https://buildinfos.debian.net/buildinfo-pool/{}/{}/{}_{}_{}.buildinfo",
            directory, self.base, self.base, version_without_epoch, arch
        );
        buildinfo_url
    }
}

#[derive(Debug, PartialEq, Eq)]
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

    fn by_maintainer(&self, maintainers: &[String]) -> bool {
        self.uploaders
            .iter()
            .any(|uploader| maintainers.iter().any(|m| uploader.starts_with(m)))
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
}

pub trait AnyhowTryFrom<T>: Sized {
    fn try_from(x: T) -> Result<Self>;
}

impl AnyhowTryFrom<NewPkg> for DebianSourcePkg {
    fn try_from(new: NewPkg) -> Result<Self> {
        Ok(DebianSourcePkg {
            base: new
                .base
                .ok_or_else(|| format_err!("Missing pkg base field"))?,
            binary: new
                .binary
                .ok_or_else(|| format_err!("Missing binary field"))?,
            version: new
                .version
                .ok_or_else(|| format_err!("Missing version field"))?,
            directory: new
                .directory
                .ok_or_else(|| format_err!("Missing directory field"))?,
            architecture: new
                .architecture
                .ok_or_else(|| format_err!("Missing architecture field"))?,
            uploaders: new.uploaders,
        })
    }
}

impl AnyhowTryFrom<NewPkg> for DebianBinPkg {
    fn try_from(new: NewPkg) -> Result<Self> {
        let name = new
            .base
            .ok_or_else(|| format_err!("Missing pkg base field"))?;
        let version = new
            .version
            .ok_or_else(|| format_err!("Missing version field"))?;

        let source = if let Some(source) = new.source {
            if let Some((name, version)) = source.split_once(' ') {
                let version = version
                    .strip_prefix('(')
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
            architecture: new
                .architecture
                .ok_or_else(|| format_err!("Missing architecture field"))?,
            filename: new
                .filename
                .ok_or_else(|| format_err!("Missing filename field"))?,
            uploaders: new.uploaders,
        })
    }
}

pub fn extract_pkgs_compressed<T: AnyhowTryFrom<NewPkg>>(bytes: &[u8]) -> Result<Vec<T>> {
    let r = XzDecoder::new(bytes);
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
            pkgs.push(T::try_from(pkg)?);
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
                            if let Ok(line) = line
                                && !line.starts_with(' ')
                            {
                                warn!("Line ended with comma, but next line is not multi-line");
                                break;
                            }
                        } else {
                            break;
                        }

                        let line = lines
                            .next()
                            .ok_or_else(|| anyhow!("Found comma on last line of file"))?
                            .context("Failed to read line")?;
                        value.push_str(&line);
                    }

                    for binary in value.split(", ") {
                        binaries.push(binary.to_string());
                    }
                    pkg.binary = Some(binaries);
                }
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
                }
                _ => (),
            }
        }
    }

    Ok(pkgs)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncState {
    reports: HashMap<(String, String, String), PackageReport>,
}

impl SyncState {
    pub fn new() -> SyncState {
        SyncState::default()
    }

    fn create_release_group(
        &mut self,
        release: &str,
        component: &str,
        architecture: &str,
    ) -> &mut PackageReport {
        let key = (
            release.to_string(),
            component.to_string(),
            architecture.to_string(),
        );
        self.reports.entry(key).or_insert_with(|| PackageReport {
            distribution: "debian".to_string(),
            release: Some(release.to_string()),
            component: Some(component.to_string()),
            architecture: architecture.to_string(),
            packages: Vec::new(),
        })
    }

    fn get_mut_group(
        &mut self,
        src: &DebianSourcePkg,
        release: &str,
        component: &str,
        architecture: &str,
    ) -> &mut SourcePackageReport {
        let report = self.create_release_group(release, component, architecture);

        match report
            .packages
            .iter()
            .position(|p| src.base == p.name && src.version == p.version)
        {
            Some(i) => &mut report.packages[i],
            None => {
                let source_report = SourcePackageReport {
                    name: src.base.clone(),
                    version: src.version.clone(),
                    url: src.buildinfo_url(architecture),
                    artifacts: Vec::new(),
                };

                report.packages.push(source_report);
                report.packages.last_mut().unwrap()
            }
        }
    }

    pub fn push(
        &mut self,
        src: &DebianSourcePkg,
        bin: DebianBinPkg,
        source: &str,
        release: &str,
        component: &str,
    ) {
        let group = self.get_mut_group(src, release, component, &bin.architecture);
        let url = format!("{}/{}", source, bin.filename);
        group.artifacts.push(BinaryPackageReport {
            name: bin.name,
            version: bin.version,
            architecture: bin.architecture.clone(),
            url,
        });
    }

    pub fn to_vec(self) -> Vec<PackageReport> {
        self.reports.into_values().collect()
    }

    pub fn import_binary_pkg(
        &mut self,
        pkg: DebianBinPkg,
        sources: &SourcePkgBucket,
        release: &str,
        component: &str,
        sync: &PkgsSync,
    ) -> Result<()> {
        // Debian combines arch:all and arch:any packages.
        // Import only what was actually specified.
        if !pkg.matches(sync) || !sync.architectures.contains(&pkg.architecture) {
            return Ok(());
        }

        debug!("Found binary package: {:?} {:?}", pkg.name, pkg.version);

        match sources.get(&pkg) {
            Ok(source) => {
                debug!(
                    "Matched binary package to source package: {:?} {:?}",
                    source.base, source.version
                );

                self.push(&source, pkg, &sync.source, release, component);
            }
            Err(e) => {
                warn!("{}, skipping", e)
            }
        }
        Ok(())
    }

    /// Ensure all release groups are created, even if we never assign any packages to it.
    /// If a release group doesn't have any packages, we still want to notify rebuilderd that
    /// it's empty.
    fn create_all_release_groups(&mut self, release: &str, component: &str, sync: &PkgsSync) {
        for arch in &sync.architectures {
            self.create_release_group(release, component, arch);
        }
    }

    pub fn import_compressed_binary_package_file(
        &mut self,
        bytes: &[u8],
        sources: &SourcePkgBucket,
        release: &str,
        component: &str,
        sync: &PkgsSync,
    ) -> Result<()> {
        self.create_all_release_groups(release, component, sync);
        for pkg in extract_pkgs_compressed::<DebianBinPkg>(bytes)? {
            self.import_binary_pkg(pkg, sources, release, component, sync)?;
        }
        Ok(())
    }

    pub fn import_uncompressed_binary_package_file(
        &mut self,
        bytes: &[u8],
        sources: &SourcePkgBucket,
        release: &str,
        component: &str,
        sync: &PkgsSync,
    ) -> Result<()> {
        self.create_all_release_groups(release, component, sync);
        for pkg in extract_pkgs_uncompressed::<DebianBinPkg, _>(bytes)? {
            self.import_binary_pkg(pkg, sources, release, component, sync)?;
        }
        Ok(())
    }
}

pub async fn sync(http: &http::Client, sync: &PkgsSync) -> Result<Vec<PackageReport>> {
    let mut state = SyncState::new();

    for release in &sync.releases {
        let mut sources = SourcePkgBucket::new();

        for component in &sync.components {
            // Downloading source package index
            let db_url = format!(
                "{}/dists/{}/{}/source/Sources.xz",
                sync.source, release, component
            );

            let bytes = fetch_url_or_path(http, &db_url).await?;

            info!("Building map of all source packages");
            sources.import_compressed_source_package_file(&bytes)?;

            for arch in &sync.architectures {
                // Downloading binary package index
                let db_url = format!(
                    "{}/dists/{}/{}/binary-{}/Packages.xz",
                    sync.source, release, component, arch
                );

                match fetch_url_or_path(http, &db_url).await {
                    Ok(bytes) => {
                        state.import_compressed_binary_package_file(
                            &bytes, &sources, release, component, sync,
                        )?;
                    }
                    Err(e) => {
                        warn!("{}, skipping", e);
                    }
                }
            }
        }
    }

    Ok(state.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_sync_empty_release() {
        let mut state = SyncState::new();
        state
            .import_uncompressed_binary_package_file(
                b"",
                &SourcePkgBucket::new(),
                "trixie-proposed-updates",
                "main",
                &PkgsSync {
                    distro: "debian".to_string(),
                    components: vec!["main".to_string()],
                    source: "http://deb.debian.org/debian".to_string(),
                    architectures: vec!["amd64".to_string()],
                    print_json: true,
                    maintainers: vec![],
                    releases: vec![],
                    pkgs: vec![],
                    excludes: vec![],
                    sync_method: None,
                },
            )
            .unwrap();
        assert_eq!(
            state.to_vec(),
            vec![PackageReport {
                distribution: "debian".to_string(),
                release: Some("trixie-proposed-updates".to_string()),
                component: Some("main".to_string()),
                architecture: "amd64".to_string(),
                packages: vec![],
            }]
        );
    }

    #[test]
    fn test_parse_bin_pkg_simple() {
        let bytes = b"Package: sniffglue
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
        assert_eq!(
            &pkgs,
            &[DebianBinPkg {
                name: "sniffglue".to_string(),
                version: "0.14.0-2".to_string(),
                source: (
                    "rust-sniffglue".to_string(),
                    VersionConstraint::Implicit("0.14.0-2".to_string())
                ),
                architecture: "amd64".to_string(),
                filename: "pool/main/r/rust-sniffglue/sniffglue_0.14.0-2_amd64.deb".to_string(),
                uploaders: vec![],
            },]
        );
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
        assert_eq!(
            &pkgs,
            &[DebianBinPkg {
                name: "mariadb-server".to_string(),
                version: "1:10.5.12-1".to_string(),
                source: (
                    "mariadb-10.5".to_string(),
                    VersionConstraint::Implicit("1:10.5.12-1".to_string())
                ),
                architecture: "all".to_string(),
                filename: "pool/main/m/mariadb-10.5/mariadb-server_10.5.12-1_all.deb".to_string(),
                uploaders: vec![],
            },]
        );
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
        assert_eq!(
            &pkgs,
            &[DebianSourcePkg {
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
            }]
        );
    }

    #[test]
    fn test_generate_group_mariadb() {
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
            source: (
                "mariadb-10.5".to_string(),
                VersionConstraint::Implicit("1:10.5.12-1".to_string()),
            ),
            architecture: "all".to_string(),
            filename: "pool/main/m/mariadb-10.5/mariadb-server_10.5.12-1_all.deb".to_string(),
            uploaders: vec![],
        };
        let mut state = SyncState::new();
        state.push(&src, bin, "https://deb.debian.org/debian", "sid", "main");

        let mut reports = HashMap::new();
        reports.insert(("sid".to_string(), "main".to_string(), "all".to_string()), PackageReport {
            distribution: "debian".to_string(),
            release: Some("sid".to_string()),
            component: Some("main".to_string()),
            architecture: "all".to_string(),
            packages: vec![
                SourcePackageReport {
                    name: "mariadb-10.5".to_string(),
                    version: "1:10.5.12-1".to_string(),
                    url: "https://buildinfos.debian.net/buildinfo-pool/m/mariadb-10.5/mariadb-10.5_10.5.12-1_all.buildinfo".to_string(),
                    artifacts: vec![
                        BinaryPackageReport {
                            name: "mariadb-server".to_string(),
                            version: "1:10.5.12-1".to_string(),
                            architecture: "all".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/m/mariadb-10.5/mariadb-server_10.5.12-1_all.deb".to_string(),
                        }
                    ],
                },
            ],
        });

        assert_eq!(state, SyncState { reports });
    }

    #[test]
    fn test_generate_group_sniffglue() {
        let bytes = "Package: librust-sniffglue-dev
Source: rust-sniffglue
Version: 0.14.0-2
Installed-Size: 317
Maintainer: Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>
Architecture: amd64
Provides: librust-sniffglue+default-dev (= 0.14.0-2), librust-sniffglue-0+default-dev (= 0.14.0-2), librust-sniffglue-0-dev (= 0.14.0-2), librust-sniffglue-0.14+default-dev (= 0.14.0-2), librust-sniffglue-0.14-dev (= 0.14.0-2), librust-sniffglue-0.14.0+default-dev (= 0.14.0-2), librust-sniffglue-0.14.0-dev (= 0.14.0-2)
Depends: librust-ansi-term-0.12+default-dev, librust-anyhow-1+default-dev, librust-atty-0.2+default-dev, librust-base64-0.13+default-dev, librust-bstr-0.2+default-dev (>= 0.2.12-~~), librust-dhcp4r-0.2+default-dev, librust-dirs-next-2+default-dev, librust-dns-parser-0.8+default-dev, librust-env-logger-0.9+default-dev, librust-libc-0.2+default-dev, librust-log-0.4+default-dev, librust-nix-0.23+default-dev, librust-nom-5+default-dev, librust-num-cpus-1+default-dev (>= 1.6-~~), librust-pcap-sys-0.1+default-dev (>= 0.1.3-~~), librust-pktparse-0.5+default-dev, librust-pktparse-0.5+serde-dev, librust-reduce-0.1+default-dev (>= 0.1.1-~~), librust-serde-1+default-dev, librust-serde-derive-1+default-dev, librust-serde-json-1+default-dev, librust-sha2-0.9+default-dev, librust-structopt-0.3+default-dev, librust-syscallz-0.15+default-dev, librust-tls-parser-0.9+default-dev, librust-toml-0.5+default-dev, librust-users-0.11+default-dev
Description: Secure multithreaded packet sniffer - Rust source code
Multi-Arch: same
Description-md5: 81e67bbd8963c4189af0a9414f889972
Section: net
Priority: optional
Filename: pool/main/r/rust-sniffglue/librust-sniffglue-dev_0.14.0-2_amd64.deb
Size: 125404
MD5sum: 6d57946f2f56b1b58d906eb29d6a021f
SHA256: c452054c216359ef44adc9a5d35870d707f47e503051dfcb736f47df17058961

Package: sniffglue
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
        let bin_pkgs = extract_pkgs_uncompressed::<DebianBinPkg, _>(cursor).unwrap();

        let bytes = "Package: rust-sniffglue
Binary: librust-sniffglue-dev, sniffglue
Version: 0.14.0-2
Maintainer: Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>
Uploaders: kpcyrd <git@rxv.cc>
Build-Depends: debhelper (>= 12), dh-cargo (>= 25), cargo:native, rustc:native, libstd-rust-dev, librust-ansi-term-0.12+default-dev, librust-anyhow-1+default-dev, librust-atty-0.2+default-dev, librust-base64-0.13+default-dev, librust-bstr-0.2+default-dev (>= 0.2.12-~~), librust-dhcp4r-0.2+default-dev, librust-dirs-next-2+default-dev, librust-dns-parser-0.8+default-dev, librust-env-logger-0.9+default-dev, librust-libc-0.2+default-dev, librust-log-0.4+default-dev, librust-nix-0.23+default-dev, librust-nom-5+default-dev, librust-num-cpus-1+default-dev (>= 1.6-~~), librust-pcap-sys-0.1+default-dev (>= 0.1.3-~~), librust-pktparse-0.5+default-dev, librust-pktparse-0.5+serde-dev, librust-reduce-0.1+default-dev (>= 0.1.1-~~), librust-serde-1+default-dev, librust-serde-derive-1+default-dev, librust-serde-json-1+default-dev, librust-sha2-0.9+default-dev, librust-structopt-0.3+default-dev, librust-syscallz-0.15+default-dev, librust-tls-parser-0.9+default-dev, librust-toml-0.5+default-dev, librust-users-0.11+default-dev
Architecture: any
Standards-Version: 4.5.1
Format: 3.0 (quilt)
Files:
 cdb6bf3fb7a8725986b313a65dd3339b 3079 rust-sniffglue_0.14.0-2.dsc
 80a7d2ab6becacf69213d9ce57d29274 134805 rust-sniffglue_0.14.0.orig.tar.gz
 9902c121e4f5cf268b19ee8b88201979 7816 rust-sniffglue_0.14.0-2.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/rust-team/debcargo-conf/tree/master/src/sniffglue
Vcs-Git: https://salsa.debian.org/rust-team/debcargo-conf.git [src/sniffglue]
Checksums-Sha256:
 b9a77f9f918769ecded338c07a344257b17d1112918b1597a8c939a719444ea4 3079 rust-sniffglue_0.14.0-2.dsc
 f056bfa09e8fae5f4cc0e1d4e8ae3619050644b321800d0d6a8cc778eb80aaf3 134805 rust-sniffglue_0.14.0.orig.tar.gz
 cb3498dd85e18e7b2c7ad5cbef2ac56e4c598df0a0ac5024aa480f97de79b096 7816 rust-sniffglue_0.14.0-2.debian.tar.xz
Package-List:
 librust-sniffglue-dev deb net optional arch=any
 sniffglue deb net optional arch=any
Testsuite: autopkgtest
Testsuite-Triggers: dh-cargo
Directory: pool/main/r/rust-sniffglue
Priority: extra
Section: misc

";
        let cursor = Cursor::new(bytes);
        let src_pkgs = extract_pkgs_uncompressed::<DebianSourcePkg, _>(cursor).unwrap();

        let mut state = SyncState::new();
        for bin in bin_pkgs {
            state.push(
                &src_pkgs[0],
                bin,
                "https://deb.debian.org/debian",
                "sid",
                "main",
            );
        }

        let mut reports = HashMap::new();
        reports.insert(("sid".to_string(), "main".to_string(), "amd64".to_string()), PackageReport {
            distribution: "debian".to_string(),
            release: Some("sid".to_string()),
            component: Some("main".to_string()),
            architecture: "amd64".to_string(),
            packages: vec![
                SourcePackageReport {
                    name: "rust-sniffglue".to_string(),
                    version: "0.14.0-2".to_string(),
                    url: "https://buildinfos.debian.net/buildinfo-pool/r/rust-sniffglue/rust-sniffglue_0.14.0-2_amd64.buildinfo".to_string(),
                    artifacts: vec![
                        BinaryPackageReport {
                            name: "librust-sniffglue-dev".to_string(),
                            version: "0.14.0-2".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/r/rust-sniffglue/librust-sniffglue-dev_0.14.0-2_amd64.deb".to_string(),
                        },
                        BinaryPackageReport {
                            name: "sniffglue".to_string(),
                            version: "0.14.0-2".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/r/rust-sniffglue/sniffglue_0.14.0-2_amd64.deb".to_string(),
                        }
                    ],
                },
            ],
        });

        assert_eq!(state, SyncState { reports });
    }

    #[test]
    fn test_generate_group_courier() {
        let bytes = "Package: courier-base
Source: courier (1.0.16-3)
Version: 1.0.16-3+b1
Installed-Size: 743
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: amd64
Replaces: courier-ssl (<< 0.75.0-1)
Depends: adduser, courier-authdaemon, courier-authlib (>= 0.66.1), courier-authlib-userdb, debconf (>= 0.5) | debconf-2.0, gnutls-bin, lsb-base, sysvinit-utils (>= 2.88dsf-50), perl:any, libc6 (>= 2.15), libcourier-unicode4 (>= 2.1.2), libgamin0 | libfam0, libgdbm6 (>= 1.16), libgnutls30 (>= 3.7.0), libidn12 (>= 1.13), libpcre3
Breaks: courier-authdaemon (<< 0.66.4-4~), courier-ssl (<< 0.75.0-1)
Description: Courier mail server - base system
Homepage: http://www.courier-mta.org/
Description-md5: 555818a698d6dfec6122cd45f5263ef2
Tag: interface::daemon, mail::transport-agent, network::server,
 role::program, works-with::mail
Section: mail
Priority: optional
Filename: pool/main/c/courier/courier-base_1.0.16-3+b1_amd64.deb
Size: 328148
MD5sum: 06d0be1d5211f0d425803633806663a5
SHA256: 688b7c11b8ec92514929d37e207681e4b9ac754db9e8cf0ab0632374433eed7e

Package: courier-doc
Source: courier
Version: 1.0.16-3
Installed-Size: 1625
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: all
Description: Courier mail server - additional documentation
Homepage: http://www.courier-mta.org/
Description-md5: 5dcceca21b36719b84d5685c5dd14e9c
Tag: interface::daemon, made-of::html, mail::transport-agent,
 role::documentation, role::program, works-with::mail
Section: doc
Priority: optional
Filename: pool/main/c/courier/courier-doc_1.0.16-3_all.deb
Size: 413660
MD5sum: efcc397d2590c11408ef0bd14f009fc7
SHA256: 8606f9cffd01b510d6f4135670b1c78fb1a9198e192cb13a9434a674702efdbc

Package: courier-faxmail
Source: courier (1.0.16-3)
Version: 1.0.16-3+b1
Installed-Size: 188
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: amd64
Depends: courier-mta (= 1.0.16-3+b1), ghostscript, groff, mgetty-fax
Recommends: netpbm
Suggests: courier-doc
Breaks: courier-webadmin (<< 1.0.14-2~)
Description: Courier mail server - Fax<->mail gateway
Homepage: http://www.courier-mta.org/
Description-md5: 1235807fe94191d8d0d1e41e90ae7d93
Tag: hardware::modem, interface::daemon, network::server, role::program,
 use::converting, works-with::fax, works-with::mail
Section: mail
Priority: optional
Filename: pool/main/c/courier/courier-faxmail_1.0.16-3+b1_amd64.deb
Size: 137492
MD5sum: 24f9a41c6380ea2ca7559ce9e6f6f54a
SHA256: 6cb78e731f845dd98ab792c43fc01a6dc3416140b08d2db00e7415eff5973527

Package: courier-imap
Source: courier (1.0.16-3)
Version: 5.0.13+1.0.16-3+b1
Installed-Size: 593
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: amd64
Replaces: courier-imap-ssl (<< 4.16.2+0.75.0-1~), imap-server
Provides: imap-server
Depends: courier-base (= 1.0.16-3+b1), debconf | debconf-2.0, gamin, default-mta | mail-transport-agent, sysvinit-utils (>= 2.88dsf-50) | init-d-script, courier-authlib (>= 0.71), libc6 (>= 2.15), libcourier-unicode4 (>= 2.1.2), libgamin0 | libfam0, libgdbm6 (>= 1.16), libidn12 (>= 1.13)
Suggests: courier-doc, imap-client
Conflicts: imap-server
Breaks: courier-imap-ssl (<< 4.16.2+0.75.0-1~)
Description: Courier mail server - IMAP server
Homepage: http://www.courier-mta.org/
Description-md5: aedad44242f18297b70663ef077f0e63
Tag: interface::daemon, mail::imap, network::server, network::service,
 protocol::imap, role::program, works-with::mail
Section: mail
Priority: optional
Filename: pool/main/c/courier/courier-imap_5.0.13+1.0.16-3+b1_amd64.deb
Size: 277952
MD5sum: f8eb4b0c42f191581189b2aee4c31793
SHA256: 67acfd8593f6c0a12a2681906e43dfe2872a694bd74e5438726646ec0e2af0a6

Package: courier-ldap
Source: courier (1.0.16-3)
Version: 1.0.16-3+b1
Installed-Size: 205
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: amd64
Replaces: courier-imap-authldap
Depends: courier-authlib-ldap, courier-base (= 1.0.16-3+b1), debconf | debconf-2.0, sysvinit-utils (>= 2.88dsf-50) | init-d-script, courier-authlib (>= 0.71), libc6 (>= 2.15), libldap-2.4-2 (>= 2.4.7)
Suggests: courier-doc
Conflicts: courier-imap-authldap
Description: Courier mail server - LDAP support
Homepage: http://www.courier-mta.org/
Description-md5: 59e644146a903a5cf765b1a1794c77bc
Tag: interface::daemon, network::server, protocol::ldap, role::program,
 security::authentication, works-with::db, works-with::mail
Section: mail
Priority: optional
Filename: pool/main/c/courier/courier-ldap_1.0.16-3+b1_amd64.deb
Size: 142284
MD5sum: fda5c8d7f98633778de1beb7197d257c
SHA256: 733c1e1f620b416fb107e115fc8fa3cbe511d1e47ce1e171e1ffb8b5b1cecd05

Package: courier-mlm
Source: courier (1.0.16-3)
Version: 1.0.16-3+b1
Installed-Size: 982
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: amd64
Replaces: sqwebmail (<< 0.75.0-1~)
Depends: courier-base (= 1.0.16-3+b1), maildrop, sysvinit-utils (>= 2.88dsf-50) | init-d-script, libc6 (>= 2.15), libcourier-unicode4 (>= 2.1.2), libgcc-s1 (>= 3.0), libgdbm6 (>= 1.16), libidn12 (>= 1.13), libstdc++6 (>= 5.2)
Suggests: courier-doc
Breaks: sqwebmail (<< 0.75.0-1~)
Description: Courier mail server - mailing list manager
Homepage: http://www.courier-mta.org/
Description-md5: fb8213625648c3fbce12df9752b51d0f
Tag: interface::daemon, mail::list, network::server, role::program,
 works-with::mail
Section: mail
Priority: optional
Filename: pool/main/c/courier/courier-mlm_1.0.16-3+b1_amd64.deb
Size: 390264
MD5sum: 4f8cc681fee4364d1a89e118c16bb5a7
SHA256: 209018400fd2dfa4caf4ede13a8865a110a9dec1c387e8eb9e45e2cd0550b771

Package: courier-mta
Source: courier (1.0.16-3)
Version: 1.0.16-3+b1
Installed-Size: 2439
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: amd64
Replaces: courier-mta-ssl (<< 0.75.0-1~), mail-transport-agent
Provides: mail-transport-agent
Depends: courier-base (= 1.0.16-3+b1), debconf (>= 0.5) | debconf-2.0, libnet-cidr-perl, sysvinit-utils (>= 2.88dsf-50) | init-d-script, wget, courier-authlib (>= 0.71), libc6 (>= 2.15), libcourier-unicode4 (>= 2.1.2), libgcc-s1 (>= 3.0), libgdbm6 (>= 1.16), libidn12 (>= 1.13), libperl5.32 (>= 5.32.0~rc1), libstdc++6 (>= 5.2), perl:any
Suggests: courier-doc, courier-filter-perl, couriergrey, mail-reader
Conflicts: mail-transport-agent
Breaks: courier-mta-ssl (<< 0.75.0-1~)
Description: Courier mail server - ESMTP daemon
Homepage: http://www.courier-mta.org/
Description-md5: 88c67d6433b0af4789d4e8a4fd1ecc42
Tag: interface::daemon, mail::transport-agent, network::server,
 network::service, protocol::smtp, role::program, works-with::mail
Section: mail
Priority: optional
Filename: pool/main/c/courier/courier-mta_1.0.16-3+b1_amd64.deb
Size: 634364
MD5sum: 788546e04c005d285cc084c1dea6b2ab
SHA256: a05a38e8aa0986067b40d1f82dbf59732c6f4697ea91d8717ef0ea88b388ae6a

Package: courier-pcp
Source: courier (1.0.16-3)
Version: 1.0.16-3+b1
Installed-Size: 245
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: amd64
Depends: sqwebmail, courier-authlib (>= 0.71), libc6 (>= 2.15)
Suggests: courier-doc
Breaks: courier-webadmin (<< 1.0.14-2~)
Description: Courier mail server - PCP server
Homepage: http://www.courier-mta.org/
Description-md5: 2607844768fa146876c3a6247b36dad9
Tag: interface::daemon, network::server, role::program, use::organizing,
 works-with::mail, works-with::pim
Section: mail
Priority: optional
Filename: pool/main/c/courier/courier-pcp_1.0.16-3+b1_amd64.deb
Size: 169328
MD5sum: 16d6b5a9f33b6da3e8cbe0fb52105aa5
SHA256: 7b118060d17983ff0a070825819a5b6ac46bad0262288e5f63c7bd0baefcf72d

Package: courier-pop
Source: courier (1.0.16-3)
Version: 1.0.16-3+b1
Installed-Size: 315
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: amd64
Replaces: courier-pop-ssl (<< 0.75.0-1~), pop3-server
Provides: pop3-server
Depends: courier-base (= 1.0.16-3+b1), debconf | debconf-2.0, default-mta | mail-transport-agent, sysvinit-utils (>= 2.88dsf-50) | init-d-script, courier-authlib (>= 0.71), libc6 (>= 2.15), libcourier-unicode4 (>= 2.1.2), libidn12 (>= 1.13)
Suggests: courier-doc, mail-reader
Conflicts: pop3-server
Breaks: courier-pop-ssl (<< 0.75.0-1~)
Description: Courier mail server - POP3 server
Homepage: http://www.courier-mta.org/
Description-md5: 89ea9794c711647b9c31923297fd27c5
Tag: interface::daemon, mail::pop, network::server, network::service,
 protocol::pop3, role::program, works-with::mail
Section: mail
Priority: optional
Filename: pool/main/c/courier/courier-pop_1.0.16-3+b1_amd64.deb
Size: 179872
MD5sum: 97bbb2d0a27f34d1b48482cfd003b081
SHA256: efc865b63f19efda4feb15917789b9390667d973ac18d7aa6e641290a7ba8461

Package: courier-webadmin
Source: courier (1.0.16-3)
Version: 1.0.16-3+b1
Installed-Size: 250
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: amd64
Depends: apache2 | httpd, courier-base (= 1.0.16-3+b1), libcgi-pm-perl, debconf (>= 0.5) | debconf-2.0, libc6 (>= 2.3.4), perl:any
Suggests: courier-doc
Description: Courier mail server - web-based administration frontend
Homepage: http://www.courier-mta.org/
Description-md5: 9d1561eee0fd4d2c370758c8fbebd3a8
Tag: admin::configuring, interface::web, role::program, use::configuring,
 web::application
Section: mail
Priority: optional
Filename: pool/main/c/courier/courier-webadmin_1.0.16-3+b1_amd64.deb
Size: 147916
MD5sum: 146605f7e774c98d94da493cc3c19c4e
SHA256: bc65ecb8eac668c0c5fe18c6784217dd67541d2347899e0096b4b2f9f2ab0059

Package: sqwebmail
Source: courier (1.0.16-3)
Version: 6.0.5+1.0.16-3+b1
Installed-Size: 1356
Maintainer: Markus Wanner <markus@bluegap.ch>
Architecture: amd64
Depends: apache2 | httpd-cgi, courier-base (= 1.0.16-3+b1), cron, debconf (>= 0.5) | debconf-2.0, default-mta | mail-transport-agent, expect, iamerican | ispell-dictionary, ispell, maildrop, sysvinit-utils (>= 2.88dsf-50) | init-d-script, courier-authlib (>= 0.71), libc6 (>= 2.15), libcourier-unicode4 (>= 2.1.2), libgamin0 | libfam0, libgdbm6 (>= 1.16), libidn12 (>= 1.13), libldap-2.4-2 (>= 2.4.7), libpcre3, perl:any
Recommends: apache2 (>= 2.4.6-4~) | httpd, courier-pcp
Suggests: courier-doc, gnupg
Description: Courier mail server - webmail server
Homepage: http://www.courier-mta.org/
Description-md5: ba58b55a2bbe8efe6abfe55d02dd61cc
Tag: interface::web, role::program, works-with::mail
Section: mail
Priority: optional
Filename: pool/main/c/courier/sqwebmail_6.0.5+1.0.16-3+b1_amd64.deb
Size: 496944
MD5sum: 5fa070104394a53254b244b16685508e
SHA256: 51ebf109a5257b34a521a68967db5b328d2301ad9e7585ccf797dfb549ed5e6e

";
        let cursor = Cursor::new(bytes);
        let bin_pkgs = extract_pkgs_uncompressed::<DebianBinPkg, _>(cursor).unwrap();

        let bytes = "Package: courier
Binary: courier-base, courier-mlm, courier-mta, courier-faxmail, courier-webadmin, sqwebmail, courier-pcp, courier-pop, courier-imap, courier-ldap, courier-doc
Version: 1.0.16-3
Maintainer: Markus Wanner <markus@bluegap.ch>
Build-Depends: automake, courier-authlib-dev (>= 0.66.4-5~), debhelper-compat (= 13), default-libmysqlclient-dev, dh-exec, dh-apache2, expect, ghostscript, gnupg2, gnutls-bin, groff-base, libcourier-unicode-dev (>= 2.1-3~), libgamin-dev, libgcrypt-dev, libgdbm-dev | libgdbmg1-dev, libgnutls28-dev, libidn11-dev, libldap2-dev, libpam0g-dev, libpcre3-dev, libperl-dev, libpq-dev, libsasl2-dev | libsasl-dev, libtool-bin | libtool, mgetty-fax, mime-support, netpbm, po-debconf, procps, wget, zlib1g-dev
Build-Conflicts: automake1.4
Architecture: any all
Standards-Version: 4.5.1
Format: 3.0 (quilt)
Files:
 dc395743184bd43c6bef647e29907439 3874 courier_1.0.16-3.dsc
 25f1c97a9ee74b7b264402b52d424ea9 7644196 courier_1.0.16.orig.tar.bz2
 f421e270aa2bb0eef69883b8dfc67661 866 courier_1.0.16.orig.tar.bz2.asc
 d82c935aa59897dc40adf85d8cc951b0 108396 courier_1.0.16-3.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/debian/courier
Vcs-Git: https://salsa.debian.org/debian/courier.git
Checksums-Sha256:
 dd89bad1059adfba65b6f06be895b97c7d3d28d4f177d6a4f055407d374ef683 3874 courier_1.0.16-3.dsc
 87fc35ddff4f273aa04f43fdffc73f9236abf39bc3234a449eab88742d885ebb 7644196 courier_1.0.16.orig.tar.bz2
 e2d574353654d2a3e473d481a1354f2d8eb6412e77277a489d6545ef41e6122d 866 courier_1.0.16.orig.tar.bz2.asc
 565912449f530457892ccee787585184d644bac76f7055698d7f41600308519f 108396 courier_1.0.16-3.debian.tar.xz
Homepage: http://www.courier-mta.org/
Package-List:
 courier-base deb mail optional arch=any
 courier-doc deb doc optional arch=all
 courier-faxmail deb mail optional arch=any
 courier-imap deb mail optional arch=any
 courier-ldap deb mail optional arch=any
 courier-mlm deb mail optional arch=any
 courier-mta deb mail optional arch=any
 courier-pcp deb mail optional arch=any
 courier-pop deb mail optional arch=any
 courier-webadmin deb mail optional arch=any
 sqwebmail deb mail optional arch=any
Testsuite: autopkgtest
Testsuite-Triggers: default-mta
Directory: pool/main/c/courier
Priority: source
Section: mail

";
        let cursor = Cursor::new(bytes);

        let mut sources = SourcePkgBucket::new();
        sources
            .import_uncompressed_source_package_file(cursor)
            .unwrap();

        let mut state = SyncState::new();
        for bin in bin_pkgs {
            let src = sources.get(&bin).unwrap();
            state.push(&src, bin, "https://deb.debian.org/debian", "sid", "main");
        }

        let mut reports = HashMap::new();
        reports.insert(("sid".to_string(), "main".to_string(), "amd64".to_string()), PackageReport {
            distribution: "debian".to_string(),
            release: Some("sid".to_string()),
            component: Some("main".to_string()),
            architecture: "amd64".to_string(),
            packages: vec![
                SourcePackageReport {
                    name: "courier".to_string(),
                    version: "1.0.16-3+b1".to_string(),
                    url: "https://buildinfos.debian.net/buildinfo-pool/c/courier/courier_1.0.16-3+b1_amd64.buildinfo".to_string(),
                    artifacts: vec![
                        BinaryPackageReport {
                            name: "courier-base".to_string(),
                            version: "1.0.16-3+b1".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/courier-base_1.0.16-3+b1_amd64.deb".to_string(),
                        },
                        BinaryPackageReport {
                            name: "courier-faxmail".to_string(),
                            version: "1.0.16-3+b1".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/courier-faxmail_1.0.16-3+b1_amd64.deb".to_string(),
                        },
                        BinaryPackageReport {
                            name: "courier-imap".to_string(),
                            version: "5.0.13+1.0.16-3+b1".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/courier-imap_5.0.13+1.0.16-3+b1_amd64.deb".to_string(),
                        },
                        BinaryPackageReport {
                            name: "courier-ldap".to_string(),
                            version: "1.0.16-3+b1".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/courier-ldap_1.0.16-3+b1_amd64.deb".to_string(),
                        },
                        BinaryPackageReport {
                            name: "courier-mlm".to_string(),
                            version: "1.0.16-3+b1".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/courier-mlm_1.0.16-3+b1_amd64.deb".to_string(),
                        },
                        BinaryPackageReport {
                            name: "courier-mta".to_string(),
                            version: "1.0.16-3+b1".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/courier-mta_1.0.16-3+b1_amd64.deb".to_string(),
                        },
                        BinaryPackageReport {
                            name: "courier-pcp".to_string(),
                            version: "1.0.16-3+b1".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/courier-pcp_1.0.16-3+b1_amd64.deb".to_string(),
                        },
                        BinaryPackageReport {
                            name: "courier-pop".to_string(),
                            version: "1.0.16-3+b1".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/courier-pop_1.0.16-3+b1_amd64.deb".to_string(),
                        },
                        BinaryPackageReport {
                            name: "courier-webadmin".to_string(),
                            version: "1.0.16-3+b1".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/courier-webadmin_1.0.16-3+b1_amd64.deb".to_string(),
                        },
                        BinaryPackageReport {
                            name: "sqwebmail".to_string(),
                            version: "6.0.5+1.0.16-3+b1".to_string(),
                            architecture: "amd64".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/sqwebmail_6.0.5+1.0.16-3+b1_amd64.deb".to_string(),
                        },
                    ],
                },
            ],
        });

        reports.insert(("sid".to_string(), "main".to_string(), "all".to_string()), PackageReport {
            distribution: "debian".to_string(),
            release: Some("sid".to_string()),
            component: Some("main".to_string()),
            architecture: "all".to_string(),
            packages: vec![
                SourcePackageReport {
                    name: "courier".to_string(),
                    version: "1.0.16-3".to_string(),
                    url: "https://buildinfos.debian.net/buildinfo-pool/c/courier/courier_1.0.16-3_all.buildinfo".to_string(),
                    artifacts: vec![
                        BinaryPackageReport {
                            name: "courier-doc".to_string(),
                            version: "1.0.16-3".to_string(),
                            architecture: "all".to_string(),
                            url: "https://deb.debian.org/debian/pool/main/c/courier/courier-doc_1.0.16-3_all.deb".to_string(),
                        }
                    ],
                },
            ],
        });

        assert_eq!(state, SyncState { reports });
    }

    #[test]
    fn test_extra_source_only() {
        let packages_bytes = "Package: python3-buildlog-consultant
Source: buildlog-consultant
Version: 0.0.37-1
Installed-Size: 6972
Maintainer: Jelmer Vernooĳ <jelmer@debian.org>
Architecture: amd64
Depends: python3-debian, python3-requirement-parser, python3-yaml, python3 (<< 3.13), python3 (>= 3.11~), python3:any, libc6 (>= 2.34), libgcc-s1 (>= 4.2)
Description: build log parser and analyser
Homepage: https://github.com/jelmer/buildlog-consultant
Description-md5: 03628aeae53788e68d12a1d9646fbb72
Section: python
Priority: optional
Filename: pool/main/b/buildlog-consultant/python3-buildlog-consultant_0.0.37-1_amd64.deb
Size: 1079688
MD5sum: 56a82bbe8d7b730c7eeb0f59e7fad6a6
SHA256: 23d9edc63238b104b42c58a82937e1e06e7351ed7df5a938029d335b3bec8e4a

Package: buildlog-consultant
Source: rust-buildlog-consultant (0.0.44-1)
Version: 0.0.44-1+b1
Installed-Size: 17441
Maintainer: Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>
Architecture: amd64
Depends: libc6 (>= 2.34), libgcc-s1 (>= 4.2)
Description: build log parser and analyser
Multi-Arch: allowed
Homepage: https://github.com/jelmer/buildlog-consultant
Built-Using: rust-lazy-regex (= 2.5.0-3), rustc (= 1.81.0+dfsg1-2)
Description-md5: 03628aeae53788e68d12a1d9646fbb72
Static-Built-Using: rust-aho-corasick (= 1.1.3-1), rust-anstream (= 0.6.15-1), rust-anstyle (= 1.0.8-1), rust-anstyle-parse (= 0.2.1-1), rust-anstyle-query (= 1.0.0-1), rust-bit-set (= 0.5.2-1), rust-bit-vec (= 0.6.3-1), rust-chrono (= 0.4.38-2), rust-clap (= 4.5.16-1), rust-clap-builder (= 4.5.15-2), rust-clap-lex (= 0.7.2-2), rust-colorchoice (= 1.0.0-1), rust-countme (= 3.0.1-1), rust-deb822-lossless (= 0.2.1-1), rust-debian-control (= 0.1.38-1), rust-debversion (= 0.4.3-2), rust-env-logger (= 0.10.2-2), rust-equivalent (= 1.0.1-1), rust-fancy-regex (= 0.11.0-2), rust-form-urlencoded (= 1.2.1-1), rust-hashbrown (= 0.14.5-5), rust-humantime (= 2.1.0-1), rust-iana-time-zone (= 0.1.60-1), rust-idna (= 0.4.0-1), rust-indexmap (= 2.2.6-1), rust-inventory (= 0.3.2-1), rust-itoa (= 1.0.9-1), rust-lazy-regex (= 2.5.0-3), rust-lazy-static (= 1.4.0-2), rust-log (= 0.4.22-1), rust-maplit (= 1.0.2-1), rust-memchr (= 2.7.4-1), rust-num-traits (= 0.2.19-2), rust-once-cell (= 1.20.2-1), rust-pep440-rs (= 0.6.6-1), rust-pep508-rs (= 0.6.1-2), rust-percent-encoding (= 2.3.1-1), rust-regex (= 1.10.6-1), rust-regex-automata (= 0.4.7-1), rust-regex-syntax (= 0.8.2-1), rust-rowan (= 0.15.16-1), rust-rustc-hash (= 1.1.0-1), rust-ryu (= 1.0.15-1), rust-serde (= 1.0.210-2), rust-serde-json (= 1.0.128-1), rust-serde-yaml (= 0.9.34-1), rust-shlex (= 1.3.0-1), rust-smallvec (= 1.13.2-1), rust-smawk (= 0.3.2-1), rust-strsim-0.10 (= 0.10.0-3), rust-termcolor (= 1.4.1-1), rust-text-size (= 1.1.1-1), rust-textwrap (= 0.16.1-1), rust-thiserror (= 1.0.59-1), rust-unicode-bidi (= 0.3.13-1), rust-unicode-linebreak (= 0.1.4-2), rust-unicode-normalization (= 0.1.22-1), rust-unicode-width (= 0.1.14-1), rust-unsafe-libyaml (= 0.2.11-1), rust-unscanny (= 0.1.0-1), rust-url (= 2.5.0-1), rust-urlencoding (= 2.1.3-1), rust-utf8parse (= 0.2.1-1), rustc (= 1.81.0+dfsg1-2)
Section: devel
Priority: optional
Filename: pool/main/r/rust-buildlog-consultant/buildlog-consultant_0.0.44-1+b1_amd64.deb
Size: 2155184
MD5sum: f396e8745f571aa6eb27ddbaf52594ad
SHA256: 7dfba1f88be2f8bdf7480085dd018333d0159ac40462daf3679f3757347262c4

";

        let sources_bytes = "Package: buildlog-consultant
Binary: python3-buildlog-consultant
Version: 0.0.37-1
Maintainer: Jelmer Vernooĳ <jelmer@debian.org>
Build-Depends: debhelper-compat (= 13), dh-sequence-python3, librust-debversion-dev, librust-fancy-regex-dev, librust-pyo3-dev, librust-pyo3-file-dev, librust-pyo3-filelike-dev, librust-pyo3-log-dev, librust-shlex-dev, librust-text-size-dev, librust-textwrap-dev, python3-all, python3-debian, python3-requirement-parser, python3-setuptools, python3-setuptools-rust, python3-yaml
Architecture: any
Standards-Version: 4.6.2
Format: 3.0 (quilt)
Files:
 6ad424f44b03af2394502b51004e715e 1999 buildlog-consultant_0.0.37-1.dsc
 94bc4b065b25128d6c2df7549ba0ce14 106321 buildlog-consultant_0.0.37.orig.tar.gz
 505fb064e8b97e075566cf654f6cd916 2784 buildlog-consultant_0.0.37-1.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/jelmer/buildlog-consultant
Vcs-Git: https://salsa.debian.org/jelmer/buildlog-consultant.git
Checksums-Sha256:
 92ce9e76fb268de73a1fa627cc9f64d97b670fed9d77ea2bbb0b7e17e33512be 1999 buildlog-consultant_0.0.37-1.dsc
 53deb11bf4e83e9f40e2df906215f55a4c6bd8299fdc07897d87ddd15f56d65a 106321 buildlog-consultant_0.0.37.orig.tar.gz
 93ae05557c211d2091cfabcc545d4a625f75c8ac3390a019c087d73528b86f4a 2784 buildlog-consultant_0.0.37-1.debian.tar.xz
Homepage: https://github.com/jelmer/buildlog-consultant
Package-List:
 python3-buildlog-consultant deb python optional arch=any
Extra-Source-Only: yes
Directory: pool/main/b/buildlog-consultant
Priority: optional
Section: misc

Package: rust-buildlog-consultant
Binary: librust-buildlog-consultant-dev, buildlog-consultant
Version: 0.0.44-1
Maintainer: Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>
Uploaders: Jelmer Vernooĳ <jelmer@debian.org>
Build-Depends: debhelper-compat (= 13), dh-cargo (>= 25), cargo:native, rustc:native, libstd-rust-dev, librust-chrono-0.4+default-dev (>= 0.4.31-~~), librust-clap-4+default-dev, librust-clap-4+derive-dev, librust-debian-control-0.1+default-dev (>= 0.1.18-~~), librust-debversion-0.4+default-dev, librust-debversion-0.4+serde-dev, librust-env-logger+default-dev (>= 0.10-~~), librust-fancy-regex-0.11+default-dev, librust-inventory-0.3+default-dev, librust-lazy-regex+default-dev (>= 2-~~), librust-lazy-static-1+default-dev, librust-log-0.4+default-dev (>= 0.4.20-~~), librust-maplit-1+default-dev (>= 1.0.2-~~), librust-pep440-rs-0.6+default-dev (>= 0.6.5-~~), librust-pep508-rs-0.6+default-dev (>= 0.6.1-~~), librust-regex-1+default-dev, librust-serde-1+default-dev, librust-serde-1+derive-dev, librust-serde-json-1+default-dev, librust-serde-yaml-0.9+default-dev, librust-shlex-1+default-dev, librust-text-size-1+default-dev (>= 1.1.1-~~), librust-textwrap-0.16+default-dev
Architecture: any
Standards-Version: 4.7.0
Format: 3.0 (quilt)
Files:
 e96dc4dbabe2027ccfffb31a47f167dd 2912 rust-buildlog-consultant_0.0.44-1.dsc
 d6de700387b2ae8ca5afb34bd9d3424d 111058 rust-buildlog-consultant_0.0.44.orig.tar.gz
 8ec7a5e281352f6401db2d75dceb8de6 3624 rust-buildlog-consultant_0.0.44-1.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/rust-team/debcargo-conf/tree/master/src/buildlog-consultant
Vcs-Git: https://salsa.debian.org/rust-team/debcargo-conf.git [src/buildlog-consultant]
Checksums-Sha256:
 0baa39785abfe865a70e456b8de6eaf48578e4bade493807e768a2026ec8ae1f 2912 rust-buildlog-consultant_0.0.44-1.dsc
 488ee699c8b6d02bde8ee3a7cb8eb9b1377f638fb34cfb6ea5a74f45d54748d9 111058 rust-buildlog-consultant_0.0.44.orig.tar.gz
 d32092503606597d71d9f39d36afd295cc675e2bf0f64f209691d50020c47061 3624 rust-buildlog-consultant_0.0.44-1.debian.tar.xz
Homepage: https://github.com/jelmer/buildlog-consultant
Package-List:
 buildlog-consultant deb devel optional arch=any
 librust-buildlog-consultant-dev deb rust optional arch=any
Testsuite: autopkgtest
Testsuite-Triggers: dh-cargo, librust-maplit-1+default-dev, rustc
Directory: pool/main/r/rust-buildlog-consultant
Priority: optional
Section: misc

Package: rust-buildlog-consultant
Binary: librust-buildlog-consultant-dev, buildlog-consultant
Version: 0.1.0-1
Maintainer: Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>
Uploaders: Jelmer Vernooĳ <jelmer@debian.org>
Build-Depends: debhelper-compat (= 13), dh-sequence-cargo, cargo:native, rustc:native, libstd-rust-dev, librust-chrono-0.4+default-dev (>= 0.4.31-~~), librust-clap-4+default-dev, librust-clap-4+derive-dev, librust-debian-control-0.1+default-dev (>= 0.1.18-~~), librust-debversion-0.4+default-dev, librust-debversion-0.4+serde-dev, librust-env-logger+default-dev (>= 0.10-~~), librust-fancy-regex-0.11+default-dev, librust-inventory-0.3+default-dev, librust-lazy-regex+default-dev (>= 2-~~), librust-lazy-static-1+default-dev, librust-log-0.4+default-dev (>= 0.4.20-~~), librust-maplit-1+default-dev (>= 1.0.2-~~), librust-pep508-rs-0.9+default-dev (>= 0.9.1-~~), librust-regex-1+default-dev, librust-serde-1+default-dev, librust-serde-1+derive-dev, librust-serde-json-1+default-dev, librust-serde-yaml-0.9+default-dev, librust-shlex-1+default-dev, librust-text-size-1+default-dev (>= 1.1.1-~~), librust-textwrap-0.16+default-dev
Architecture: any
Standards-Version: 4.7.0
Format: 3.0 (quilt)
Files:
 7f4d52474fdeb91fd28b30265c2951a0 2881 rust-buildlog-consultant_0.1.0-1.dsc
 780a2b95f0b58b8607811f8c2a3ed5b3 112422 rust-buildlog-consultant_0.1.0.orig.tar.gz
 a83d2bdc040e133428a3d08d1beb6511 3716 rust-buildlog-consultant_0.1.0-1.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/rust-team/debcargo-conf/tree/master/src/buildlog-consultant
Vcs-Git: https://salsa.debian.org/rust-team/debcargo-conf.git [src/buildlog-consultant]
Checksums-Sha256:
 d866b60e266258cd49cc0b5e0eef4328469ac2001c5d470564ac8aca37768539 2881 rust-buildlog-consultant_0.1.0-1.dsc
 e18d1855ddb4376d5dc6e34c6e1ae818c46437092d56ea5c559358548d5be842 112422 rust-buildlog-consultant_0.1.0.orig.tar.gz
 54bbbec4464f413d059406c93ffe3fa92e1a7b3385c8dfbcf5ce7d0b2a236c93 3716 rust-buildlog-consultant_0.1.0-1.debian.tar.xz
Homepage: https://github.com/jelmer/buildlog-consultant
Package-List:
 buildlog-consultant deb devel optional arch=any
 librust-buildlog-consultant-dev deb rust optional arch=any
Testsuite: autopkgtest
Testsuite-Triggers: dh-cargo, librust-maplit-1+default-dev, rustc
Directory: pool/main/r/rust-buildlog-consultant
Priority: optional
Section: misc

";

        let mut sources = SourcePkgBucket::new();
        sources
            .import_uncompressed_source_package_file(sources_bytes.as_bytes())
            .unwrap();

        let pkgs = extract_pkgs_uncompressed::<DebianBinPkg, _>(packages_bytes.as_bytes()).unwrap();
        let mut pkgs = pkgs.into_iter();

        // test first package (with Extra-Source-Only)
        let pkg = pkgs.next().unwrap();
        let src = sources.get(&pkg).unwrap();
        assert_eq!(
            src,
            DebianSourcePkg {
                base: "buildlog-consultant".to_string(),
                binary: vec!["python3-buildlog-consultant".to_string(),],
                version: "0.0.37-1".to_string(),
                directory: "pool/main/b/buildlog-consultant".to_string(),
                architecture: "any".to_string(),
                uploaders: vec![],
            }
        );

        // test second package (without Extra-Source-Only)
        let pkg = pkgs.next().unwrap();
        let src = sources.get(&pkg).unwrap();
        assert_eq!(
            src,
            DebianSourcePkg {
                base: "rust-buildlog-consultant".to_string(),
                binary: vec![
                    "librust-buildlog-consultant-dev".to_string(),
                    "buildlog-consultant".to_string(),
                ],
                version: "0.0.44-1+b1".to_string(),
                directory: "pool/main/r/rust-buildlog-consultant".to_string(),
                architecture: "any".to_string(),
                uploaders: vec!["Jelmer Vernooĳ <jelmer@debian.org>".to_string()],
            }
        );

        assert_eq!(pkgs.next(), None);
    }

    #[test]
    fn test_no_duplicate_binary_pkgs() {
        let sources_bytes = b"Package: rust-repro-env
Binary: librust-repro-env-dev, repro-env
Version: 0.4.3-2
Maintainer: Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>
Uploaders: kpcyrd <git@rxv.cc>
Build-Depends: debhelper-compat (= 13), dh-sequence-cargo, bash-completion, scdoc
Architecture: any
Standards-Version: 4.7.0
Format: 3.0 (quilt)
Files:
 cb3be021f1a26c8b4deff5ec1b46c461 3900 rust-repro-env_0.4.3-2.dsc
 2e0b94e6fe008835bf8b7eb69bcfda25 73123 rust-repro-env_0.4.3.orig.tar.gz
 95c233e26220f0f8c3c9c3f4da300974 4588 rust-repro-env_0.4.3-2.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/rust-team/debcargo-conf/tree/master/src/repro-env
Vcs-Git: https://salsa.debian.org/rust-team/debcargo-conf.git [src/repro-env]
Checksums-Sha256:
 9dd298d34d5a4f45686cdc00255519546e3ab788e7177853485b0d15696079ce 3900 rust-repro-env_0.4.3-2.dsc
 d11b99832a0d3917b3f0d1a2f95729b22492921a07939d58451cb2a7808624c7 73123 rust-repro-env_0.4.3.orig.tar.gz
 fdea8a698b0228a4787a7a1c59347d25ae367e46e320bffe302095a888d6fbea 4588 rust-repro-env_0.4.3-2.debian.tar.xz
Homepage: https://github.com/kpcyrd/repro-env
Build-Depends-Arch: cargo:native, rustc:native, libstd-rust-dev, librust-anyhow-1+default-dev (>= 1.0.71-~~), librust-ar-0.9+default-dev, librust-bytes-1+default-dev (>= 1.4.0-~~), librust-clap-4+default-dev, librust-clap-4+derive-dev, librust-clap-complete-4+default-dev, librust-clone-file-0.1+default-dev, librust-data-encoding-2+default-dev (>= 2.4.0-~~), librust-dirs-5+default-dev, librust-env-logger-0.11+default-dev, librust-fd-lock-3+default-dev, librust-flate2-1+default-dev (>= 1.0.26-~~), librust-hex-0.4+default-dev (>= 0.4.3-~~), librust-indexmap-2+default-dev (>= 2.1.0-~~), librust-indexmap-2+serde-dev (>= 2.1.0-~~), librust-log-0.4+default-dev (>= 0.4.19-~~), librust-lz4-flex-0.11+default-dev (>= 0.11.1-~~), librust-lzma-rs-0.3+default-dev, librust-memchr-2+default-dev (>= 2.5.0-~~), librust-nix-0.29+sched-dev, librust-peekread-0.1+default-dev (>= 0.1.1-~~), librust-reqwest-0.12+rustls-tls-native-roots-dev (>= 0.12.12-~~), librust-reqwest-0.12+socks-dev (>= 0.12.12-~~), librust-reqwest-0.12+stream-dev (>= 0.12.12-~~), librust-ruzstd-0.7+default-dev, librust-sequoia-openpgp-2+default-dev, librust-serde-1+default-dev, librust-serde-1+derive-dev, librust-serde-json-1+default-dev, librust-sha1-0.10+default-dev (>= 0.10.5-~~), librust-sha2-0.10+default-dev (>= 0.10.7-~~), librust-tar-0.4+default-dev (>= 0.4.38-~~), librust-tempfile-3+default-dev (>= 3.6.0-~~), librust-time-0.3+default-dev, librust-time-0.3+formatting-dev, librust-tokio-1+default-dev, librust-tokio-1+fs-dev, librust-tokio-1+macros-dev, librust-tokio-1+process-dev, librust-tokio-1+rt-multi-thread-dev, librust-tokio-1+signal-dev, librust-toml-0.8+default-dev, librust-urlencoding-2+default-dev (>= 2.1.2-~~)
Package-List:
 librust-repro-env-dev deb utils optional arch=any
 repro-env deb utils optional arch=any
Testsuite: autopkgtest
Testsuite-Triggers: dh-cargo, rustc
Directory: pool/main/r/rust-repro-env
Priority: optional
Section: misc

";
        let mut sources = SourcePkgBucket::new();
        sources
            .import_uncompressed_source_package_file(&sources_bytes[..])
            .unwrap();

        let bytes = b"Package: repro-env
Source: rust-repro-env
Version: 0.4.3-2
Installed-Size: 9307
Maintainer: Debian Rust Maintainers <pkg-rust-maintainers@alioth-lists.debian.net>
Architecture: amd64
Depends: libbz2-1.0, libc6 (>= 2.39), libgcc-s1 (>= 4.2), libgmp10 (>= 2:6.3.0+dfsg), libhogweed6t64, libnettle8t64 (>= 3.9~), catatonit, podman
Recommends: ca-certificates
Description: Dependency lockfiles for reproducible build environments
Multi-Arch: allowed
Homepage: https://github.com/kpcyrd/repro-env
Built-Using: rust-buffered-reader (= 1.3.1-2), rust-nettle (= 7.3.0-1), rust-nettle-sys (= 2.3.1-1), rust-nix (= 0.29.0-2), rust-option-ext (= 0.2.0-1), rust-rustls (= 0.23.25+ds-1), rust-rustls-native-certs (= 0.6.3-7), rust-rustls-pki-types (= 1.11.0-2), rust-rustls-webpki (= 0.102.8-5), rust-sequoia-openpgp (= 2.0.0-1), rustc (= 1.85.0+dfsg1-1)
Description-md5: f863ee24111d78d4e61cd98f092e44df
Static-Built-Using: rust-adler (= 1.0.2-2), rust-aho-corasick (= 1.1.3-1), rust-anstream (= 0.6.15-1), rust-anstyle (= 1.0.8-1), rust-anstyle-parse (= 0.2.1-1), rust-anstyle-query (= 1.0.0-1), rust-anyhow (= 1.0.95-1), rust-ar (= 0.9.0-1), rust-argon2 (= 0.5.3-1), rust-base64 (= 0.22.1-1), rust-base64ct (= 1.6.0-1), rust-bitflags (= 2.8.0-1), rust-blake2 (= 0.10.6-2), rust-block-buffer (= 0.10.4-1), rust-buffered-reader (= 1.3.1-2), rust-byteorder (= 1.5.0-1), rust-bytes (= 1.9.0-1), rust-bzip2 (= 0.4.4-1), rust-bzip2-sys (= 0.1.11-1), rust-cfg-if (= 1.0.0-1), rust-clap (= 4.5.23-1), rust-clap-builder (= 4.5.23-1), rust-clap-complete (= 4.5.40-1), rust-clap-lex (= 0.7.4-1), rust-clone-file (= 0.1.0-3), rust-colorchoice (= 1.0.0-1), rust-cpufeatures (= 0.2.16-1), rust-crc (= 3.2.1-1), rust-crc-catalog (= 2.4.0-1), rust-crc32fast (= 1.4.2-1), rust-crypto-common (= 0.1.6-1), rust-data-encoding (= 2.6.0-1), rust-deranged (= 0.3.11-1), rust-digest (= 0.10.7-2), rust-dirs (= 5.0.1-1), rust-dirs-sys (= 0.4.1-1), rust-dyn-clone (= 1.0.16-1), rust-either (= 1.13.0-1), rust-env-filter (= 0.1.3-1), rust-env-logger (= 0.11.6-1), rust-equivalent (= 1.0.1-1), rust-fastrand (= 2.1.1-1), rust-fd-lock (= 3.0.13-1), rust-filetime (= 0.2.24-1), rust-flate2 (= 1.1.0-1), rust-fnv (= 1.0.7-1), rust-form-urlencoded (= 1.2.1-1), rust-futures-channel (= 0.3.31-1), rust-futures-core (= 0.3.31-1), rust-futures-sink (= 0.3.31-1), rust-futures-task (= 0.3.31-3), rust-futures-util (= 0.3.31-1), rust-generic-array (= 0.14.7-1), rust-getrandom (= 0.2.15-1), rust-hashbrown (= 0.14.5-5), rust-hex (= 0.4.3-2), rust-http (= 1.2.0-1), rust-http-body (= 1.0.1-1), rust-http-body-util (= 0.1.2-1), rust-httparse (= 1.8.0-1), rust-humantime (= 2.1.0-2), rust-hyper (= 1.5.2-1), rust-hyper-util (= 0.1.10-1), rust-idna (= 0.4.0-1), rust-indexmap (= 2.7.0-1), rust-ipnet (= 2.11.0-1), rust-itoa (= 1.0.14-1), rust-lalrpop-util (= 0.20.0-1), rust-lazy-static (= 1.5.0-1), rust-libc (= 0.2.169-1), rust-linux-raw-sys (= 0.4.14-1), rust-log (= 0.4.26-1), rust-lz4-flex (= 0.11.3-1), rust-lzma-rs (= 0.3.0-3), rust-memchr (= 2.7.4-1), rust-memsec (= 0.7.0-1), rust-mime (= 0.3.17-1), rust-miniz-oxide (= 0.7.1-1), rust-mio (= 1.0.2-3), rust-nettle (= 7.3.0-1), rust-nettle-sys (= 2.3.1-1), rust-nix (= 0.29.0-2), rust-num-conv (= 0.1.0-1), rust-once-cell (= 1.20.2-1), rust-openssl-probe (= 0.1.2-1), rust-option-ext (= 0.2.0-1), rust-password-hash (= 0.5.0-1), rust-peekread (= 0.1.1-1), rust-percent-encoding (= 2.3.1-1), rust-pin-project-lite (= 0.2.13-1), rust-pin-utils (= 0.1.0-1), rust-powerfmt (= 0.2.0-1), rust-rand-core (= 0.6.4-2), rust-regex (= 1.11.1-2), rust-regex-automata (= 0.4.9-2), rust-regex-syntax (= 0.8.5-1), rust-reqwest (= 0.12.12-3), rust-ring (= 0.17.8-2), rust-rustix (= 0.38.37-1), rust-rustls (= 0.23.25+ds-1), rust-rustls-native-certs (= 0.6.3-7), rust-rustls-pemfile (= 2.2.0-3), rust-rustls-pki-types (= 1.11.0-2), rust-rustls-webpki (= 0.102.8-5), rust-ruzstd (= 0.7.3-2), rust-ryu (= 1.0.19-1), rust-sequoia-openpgp (= 2.0.0-1), rust-serde (= 1.0.217-1), rust-serde-json (= 1.0.139-1), rust-serde-spanned (= 0.6.7-1), rust-serde-urlencoded (= 0.7.1-1), rust-sha1 (= 0.10.6-1), rust-sha1collisiondetection (= 0.3.2-1), rust-sha2 (= 0.10.8-1), rust-signal-hook-registry (= 1.4.0-1), rust-smallvec (= 1.13.2-1), rust-socket2 (= 0.5.8-1), rust-spin (= 0.9.8-4), rust-static-assertions (= 1.1.0-1), rust-strsim-0.10 (= 0.10.0-4), rust-subtle (= 2.6.1-1), rust-sync-wrapper (= 1.0.2-1), rust-tar (= 0.4.43-4), rust-tempfile (= 3.15.0-1), rust-thiserror (= 2.0.11-1), rust-thiserror-1 (= 1.0.69-1), rust-time (= 0.3.37-1), rust-time-core (= 0.1.2-1), rust-tinyvec (= 1.6.0-2), rust-tinyvec-macros (= 0.1.0-1), rust-tokio (= 1.43.0-1), rust-tokio-rustls (= 0.26.1-2), rust-tokio-socks (= 0.5.2-1), rust-tokio-util (= 0.7.10-1), rust-toml (= 0.8.19-1), rust-toml-datetime (= 0.6.8-1), rust-toml-edit (= 0.22.22-1), rust-tower (= 0.5.2-2), rust-tower-layer (= 0.3.3-1), rust-tower-service (= 0.3.3-1), rust-tracing (= 0.1.40-1), rust-tracing-core (= 0.1.32-1), rust-try-lock (= 0.2.5-1), rust-twox-hash (= 1.6.3-1), rust-typenum (= 1.17.0-2), rust-unicode-bidi (= 0.3.17-1), rust-unicode-normalization (= 0.1.24-1), rust-untrusted (= 0.9.0-2), rust-url (= 2.5.2-1), rust-urlencoding (= 2.1.3-1), rust-utf8parse (= 0.2.1-1), rust-want (= 0.3.0-1), rust-winnow (= 0.6.26-1), rust-xattr (= 1.3.1-1), rust-xxhash-rust (= 0.8.15-1), rust-zeroize (= 1.8.1-1), rustc (= 1.85.0+dfsg1-1)
Section: utils
Priority: optional
Filename: pool/main/r/rust-repro-env/repro-env_0.4.3-2_amd64.deb
Size: 2729904
MD5sum: c740ad64bda01ccf932fe613baf02acf
SHA256: cc2081a6b2f6dcb82039b5097405b5836017a7bfc54a78eba36b656549e17c92

";

        let mut state = SyncState::new();
        let sync = PkgsSync {
            distro: "debian".to_string(),
            components: vec!["main".to_string()],
            source: "http://deb.debian.org/debian".to_string(),
            architectures: vec!["amd64".to_string()],
            print_json: true,
            maintainers: vec![],
            releases: vec!["sid".to_string(), "testing".to_string()],
            pkgs: vec![],
            excludes: vec![],
            sync_method: None,
        };

        // add the package list twice, to simulate importing sid and testing
        state
            .import_uncompressed_binary_package_file(&bytes[..], &sources, "sid", "main", &sync)
            .unwrap();
        state
            .import_uncompressed_binary_package_file(&bytes[..], &sources, "testing", "main", &sync)
            .unwrap();

        let mut reports = HashMap::new();
        reports.insert(("sid".to_string(), "main".to_string(), "amd64".to_string()), PackageReport {
            distribution: "debian".to_string(),
            release: Some("sid".to_string()),
            component: Some("main".to_string()),
            architecture: "amd64".to_string(),
            packages: vec![
                SourcePackageReport {
                    name: "rust-repro-env".to_string(),
                    version: "0.4.3-2".to_string(),
                    url: "https://buildinfos.debian.net/buildinfo-pool/r/rust-repro-env/rust-repro-env_0.4.3-2_amd64.buildinfo".to_string(),
                    artifacts: vec![
                        BinaryPackageReport {
                            name: "repro-env".to_string(),
                            version: "0.4.3-2".to_string(),
                            architecture: "amd64".to_string(),
                            url: "http://deb.debian.org/debian/pool/main/r/rust-repro-env/repro-env_0.4.3-2_amd64.deb".to_string(),
                        },
                    ]
                }
            ],
        });

        reports.insert(("testing".to_string(), "main".to_string(), "amd64".to_string()), PackageReport {
            distribution: "debian".to_string(),
            release: Some("testing".to_string()),
            component: Some("main".to_string()),
            architecture: "amd64".to_string(),
            packages: vec![
                SourcePackageReport {
                    name: "rust-repro-env".to_string(),
                    version: "0.4.3-2".to_string(),
                    url: "https://buildinfos.debian.net/buildinfo-pool/r/rust-repro-env/rust-repro-env_0.4.3-2_amd64.buildinfo".to_string(),
                    artifacts: vec![
                        BinaryPackageReport {
                            name: "repro-env".to_string(),
                            version: "0.4.3-2".to_string(),
                            architecture: "amd64".to_string(),
                            url: "http://deb.debian.org/debian/pool/main/r/rust-repro-env/repro-env_0.4.3-2_amd64.deb".to_string(),
                        },
                    ]
                }
            ],
        });

        assert_eq!(state, SyncState { reports });
    }

    #[test]
    fn test_novnc_partially_pending_decruft() {
        let mut state = SyncState::new();

        let sync = PkgsSync {
            distro: "debian".to_string(),
            components: vec!["main".to_string()],
            source: "http://deb.debian.org/debian".to_string(),
            architectures: vec!["amd64".to_string(), "all".to_string()],
            print_json: true,
            maintainers: vec![],
            releases: vec!["sid".to_string(), "testing".to_string()],
            pkgs: vec![],
            excludes: vec![],
            sync_method: None,
        };

        // sid
        let (source, binary) = (
&b"Package: novnc
Binary: novnc, python3-novnc
Version: 1:1.6.0-1
Maintainer: Debian OpenStack <team+openstack@tracker.debian.org>
Uploaders: Thomas Goirand <zigo@debian.org>, Michal Arbet <michal.arbet@ultimum.io>,
Build-Depends: debhelper-compat (= 11), dh-python, openstack-pkg-tools, python3-all:any, python3-setuptools
Build-Depends-Indep: gettext, node-commander, node-po2json, python3-greenlet
Architecture: all
Standards-Version: 4.6.0
Format: 3.0 (quilt)
Files:
 602d3ac601a5b8a86fdee458db309ffe 2138 novnc_1.6.0-1.dsc
 776338137022afc9f7017e92713def0a 622076 novnc_1.6.0.orig.tar.xz
 a231e8f85a415ae25aed045b443ac79d 37840 novnc_1.6.0-1.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/openstack-team/third-party/novnc
Vcs-Git: https://salsa.debian.org/openstack-team/third-party/novnc.git
Checksums-Sha256:
 47de3f653e63123770c1c45b34b7da476fd6c2aa0e258dffbac60eb307859b97 2138 novnc_1.6.0-1.dsc
 136fb59977cdc7eab902097a16ccd94669b32d5c34dc035c52eae9b4ab8d77f6 622076 novnc_1.6.0.orig.tar.xz
 df38150ece65f2c726edcfe56d65d70c30d313450a2ef84b735be82b196ef768 37840 novnc_1.6.0-1.debian.tar.xz
Homepage: https://github.com/novnc/noVNC
Package-List:
 novnc deb web optional arch=all
 python3-novnc deb python optional arch=all
Directory: pool/main/n/novnc
Priority: source
Section: web

Package: novnc
Binary: novnc
Version: 1:1.6.0-2
Maintainer: Debian OpenStack <team+openstack@tracker.debian.org>
Uploaders: Thomas Goirand <zigo@debian.org>, Michal Arbet <michal.arbet@ultimum.io>,
Build-Depends: debhelper-compat (= 11), dh-python, openstack-pkg-tools
Build-Depends-Indep: gettext, node-commander, node-po2json
Architecture: all
Standards-Version: 4.6.0
Format: 3.0 (quilt)
Files:
 fd3c71dc3356432dfd98d71e7f873ff6 2024 novnc_1.6.0-2.dsc
 776338137022afc9f7017e92713def0a 622076 novnc_1.6.0.orig.tar.xz
 250801b18122eb4bb139477a95adb887 37728 novnc_1.6.0-2.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/openstack-team/third-party/novnc
Vcs-Git: https://salsa.debian.org/openstack-team/third-party/novnc.git
Checksums-Sha256:
 15e5606aa4f05d0d6c721b18e96e1d5fab4853cb8fbd28e0d0b4687e5a6f81b7 2024 novnc_1.6.0-2.dsc
 136fb59977cdc7eab902097a16ccd94669b32d5c34dc035c52eae9b4ab8d77f6 622076 novnc_1.6.0.orig.tar.xz
 6bcfffafd44a02344d25f16adb367a9f65d9d2105564ef2356284485a84e98a0 37728 novnc_1.6.0-2.debian.tar.xz
Homepage: https://github.com/novnc/noVNC
Package-List:
 novnc deb web optional arch=all
Directory: pool/main/n/novnc
Priority: source
Section: web

"[..], &b"Package: novnc
Version: 1:1.6.0-2
Installed-Size: 1444
Maintainer: Debian OpenStack <team+openstack@tracker.debian.org>
Architecture: all
Depends: adduser, nodejs, net-tools, websockify
Suggests: python-nova
Description: HTML5 VNC client - daemon and programs
Homepage: https://github.com/novnc/noVNC
Description-md5: 3ea408da6df8fcf109db7d2cceeba20a
Section: web
Priority: optional
Filename: pool/main/n/novnc/novnc_1.6.0-2_all.deb
Size: 280152
MD5sum: 4feccfc563133e7345f02382fc5534d9
SHA256: 7943751137815b9b98c7b424413de78aefa8a1045129ac06c001e9e68e0de98e

Package: python3-novnc
Source: novnc
Version: 1:1.6.0-1
Installed-Size: 49
Maintainer: Debian OpenStack <team+openstack@tracker.debian.org>
Architecture: all
Depends: python3-oslo.config, python3-pil, python3:any
Suggests: python3-nova
Description: HTML5 VNC client - Python 3.x libraries
Homepage: https://github.com/novnc/noVNC
Description-md5: b0c6a6afac84b7f718cfbd184ca21a58
Section: python
Priority: optional
Filename: pool/main/n/novnc/python3-novnc_1.6.0-1_all.deb
Size: 12664
MD5sum: e088e49616de39f4cfa162959335340e
SHA256: 89c378d37058ea2a6c5d4bb2c1d47c4810f7504bde9e4d8142ac9781ce9df002

"[..]);
        let mut sources = SourcePkgBucket::new();
        sources
            .import_uncompressed_source_package_file(source)
            .unwrap();
        state
            .import_uncompressed_binary_package_file(binary, &sources, "sid", "main", &sync)
            .unwrap();

        // testing
        let (source, binary) = (
b"Package: novnc
Binary: novnc, python3-novnc
Version: 1:1.6.0-1
Maintainer: Debian OpenStack <team+openstack@tracker.debian.org>
Uploaders: Thomas Goirand <zigo@debian.org>, Michal Arbet <michal.arbet@ultimum.io>,
Build-Depends: debhelper-compat (= 11), dh-python, openstack-pkg-tools, python3-all:any, python3-setuptools
Build-Depends-Indep: gettext, node-commander, node-po2json, python3-greenlet
Architecture: all
Standards-Version: 4.6.0
Format: 3.0 (quilt)
Files:
 602d3ac601a5b8a86fdee458db309ffe 2138 novnc_1.6.0-1.dsc
 776338137022afc9f7017e92713def0a 622076 novnc_1.6.0.orig.tar.xz
 a231e8f85a415ae25aed045b443ac79d 37840 novnc_1.6.0-1.debian.tar.xz
Vcs-Browser: https://salsa.debian.org/openstack-team/third-party/novnc
Vcs-Git: https://salsa.debian.org/openstack-team/third-party/novnc.git
Checksums-Sha256:
 47de3f653e63123770c1c45b34b7da476fd6c2aa0e258dffbac60eb307859b97 2138 novnc_1.6.0-1.dsc
 136fb59977cdc7eab902097a16ccd94669b32d5c34dc035c52eae9b4ab8d77f6 622076 novnc_1.6.0.orig.tar.xz
 df38150ece65f2c726edcfe56d65d70c30d313450a2ef84b735be82b196ef768 37840 novnc_1.6.0-1.debian.tar.xz
Homepage: https://github.com/novnc/noVNC
Package-List:
 novnc deb web optional arch=all
 python3-novnc deb python optional arch=all
Directory: pool/main/n/novnc
Priority: source
Section: web

", b"Package: novnc
Version: 1:1.6.0-1
Installed-Size: 1444
Maintainer: Debian OpenStack <team+openstack@tracker.debian.org>
Architecture: all
Depends: adduser, nodejs, net-tools, python3-novnc, python3-numpy, websockify
Suggests: python-nova
Description: HTML5 VNC client - daemon and programs
Homepage: https://github.com/novnc/noVNC
Description-md5: 3ea408da6df8fcf109db7d2cceeba20a
Section: web
Priority: optional
Filename: pool/main/n/novnc/novnc_1.6.0-1_all.deb
Size: 280008
MD5sum: e5dfdf48658a966ed211a5acbbf30cc2
SHA256: 8fe336a27f2054d4763969395de893406bf06b5c34a46b72cb1de1928511ad02

Package: python3-novnc
Source: novnc
Version: 1:1.6.0-1
Installed-Size: 49
Maintainer: Debian OpenStack <team+openstack@tracker.debian.org>
Architecture: all
Depends: python3-oslo.config, python3-pil, python3:any
Suggests: python3-nova
Description: HTML5 VNC client - Python 3.x libraries
Homepage: https://github.com/novnc/noVNC
Description-md5: b0c6a6afac84b7f718cfbd184ca21a58
Section: python
Priority: optional
Filename: pool/main/n/novnc/python3-novnc_1.6.0-1_all.deb
Size: 12664
MD5sum: e088e49616de39f4cfa162959335340e
SHA256: 89c378d37058ea2a6c5d4bb2c1d47c4810f7504bde9e4d8142ac9781ce9df002

");
        let mut sources = SourcePkgBucket::new();
        sources
            .import_uncompressed_source_package_file(&source[..])
            .unwrap();
        state
            .import_uncompressed_binary_package_file(binary, &sources, "testing", "main", &sync)
            .unwrap();

        let mut reports = HashMap::new();

        reports.insert(("sid".to_string(), "main".to_string(), "all".to_string()),
           PackageReport {
               distribution: "debian".to_string(),
               release: Some("sid".to_string()),
               component: Some("main".to_string()),
               architecture: "all".to_string(),
               packages: vec![
                   SourcePackageReport {
                       name: "novnc".to_string(),
                       version: "1:1.6.0-2".to_string(),
                       url: "https://buildinfos.debian.net/buildinfo-pool/n/novnc/novnc_1.6.0-2_all.buildinfo".to_string(),
                       artifacts: vec![
                           BinaryPackageReport {
                               name: "novnc".to_string(),
                               version: "1:1.6.0-2".to_string(),
                               architecture: "all".to_string(),
                               url: "http://deb.debian.org/debian/pool/main/n/novnc/novnc_1.6.0-2_all.deb".to_string(),
                           },
                       ]
                   },
                   SourcePackageReport {
                       name: "novnc".to_string(),
                       version: "1:1.6.0-1".to_string(),
                       url: "https://buildinfos.debian.net/buildinfo-pool/n/novnc/novnc_1.6.0-1_all.buildinfo".to_string(),
                       artifacts: vec![
                           BinaryPackageReport {
                               name: "python3-novnc".to_string(),
                               version: "1:1.6.0-1".to_string(),
                               architecture: "all".to_string(),
                               url: "http://deb.debian.org/debian/pool/main/n/novnc/python3-novnc_1.6.0-1_all.deb".to_string(),
                           },
                       ]
                   },
               ],
           });

        reports.insert(("testing".to_string(), "main".to_string(), "all".to_string()),
           PackageReport {
               distribution: "debian".to_string(),
               release: Some("testing".to_string()),
               component: Some("main".to_string()),
               architecture: "all".to_string(),
               packages: vec![
                   SourcePackageReport {
                       name: "novnc".to_string(),
                       version: "1:1.6.0-1".to_string(),
                       url: "https://buildinfos.debian.net/buildinfo-pool/n/novnc/novnc_1.6.0-1_all.buildinfo".to_string(),
                       artifacts: vec![
                           BinaryPackageReport {
                               name: "novnc".to_string(),
                               version: "1:1.6.0-1".to_string(),
                               architecture: "all".to_string(),
                               url: "http://deb.debian.org/debian/pool/main/n/novnc/novnc_1.6.0-1_all.deb".to_string(),
                           },
                           BinaryPackageReport {
                               name: "python3-novnc".to_string(),
                               version: "1:1.6.0-1".to_string(),
                               architecture: "all".to_string(),
                               url: "http://deb.debian.org/debian/pool/main/n/novnc/python3-novnc_1.6.0-1_all.deb".to_string(),
                           },
                       ]
                   },
               ],
           },
        );

        reports.insert(
            ("sid".to_string(), "main".to_string(), "amd64".to_string()),
            PackageReport {
                distribution: "debian".to_string(),
                release: Some("sid".to_string()),
                component: Some("main".to_string()),
                architecture: "amd64".to_string(),
                packages: vec![],
            },
        );

        reports.insert(
            (
                "testing".to_string(),
                "main".to_string(),
                "amd64".to_string(),
            ),
            PackageReport {
                distribution: "debian".to_string(),
                release: Some("testing".to_string()),
                component: Some("main".to_string()),
                architecture: "amd64".to_string(),
                packages: vec![],
            },
        );

        assert_eq!(state, SyncState { reports });
    }
}
