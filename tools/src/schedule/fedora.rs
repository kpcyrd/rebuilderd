use crate::args::PkgsSync;
use crate::decompress;
use crate::schedule::{fetch_url_or_path, Pkg};
use rebuilderd_common::api::v1::{BinaryPackageReport, PackageReport, SourcePackageReport};
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

pub async fn sync(http: &http::Client, sync: &PkgsSync) -> Result<Vec<PackageReport>> {
    let mut reports = Vec::new();

    for release in &sync.releases {
        for arch in &sync.architectures {
            let url = format!("{}/{}/{}/{}/os/", sync.source, release, sync.suite, arch);
            let bytes = fetch_url_or_path(http, &format!("{url}repodata/repomd.xml")).await?;
            let location = get_primary_location_from_xml(&bytes)?;

            let bytes = fetch_url_or_path(http, &format!("{url}{location}")).await?;
            info!("Parsing index ({} bytes)...", bytes.len());

            let comp = decompress::detect_compression(&bytes);
            let data = decompress::stream(comp, &bytes)?;
            let packages = parse_package_index(data)?;

            let mut report = PackageReport {
                distribution: "archlinux".to_string(),
                release: None,
                component: Some(sync.suite.clone()),
                architecture: arch.clone(),
                packages: Vec::new(),
            };

            let mut bases: HashMap<_, SourcePackageReport> = HashMap::new();

            for pkg in packages {
                if !pkg.matches(sync) {
                    continue;
                }

                let url = format!("{url}{}", pkg.location.href);
                let version = format!("{}-{}", pkg.version.ver, pkg.version.rel);
                let artifact = BinaryPackageReport {
                    name: pkg.name,
                    version,
                    architecture: pkg.arch,
                    url: url.clone(),
                };

                if let Some(group) = bases.get_mut(&pkg.format.sourcerpm) {
                    group.artifacts.push(artifact);
                } else {
                    let mut group = SourcePackageReport {
                        name: pkg.format.sourcerpm.clone(),
                        version: format!("{}-{}", pkg.version.ver, pkg.version.rel),
                        url: url.clone(), // use first artifact's url as the source URL for now
                        artifacts: Vec::new(),
                    };

                    group.artifacts.push(artifact);
                    bases.insert(pkg.format.sourcerpm, group);
                }
            }

            report.packages = bases.into_values().collect();
            reports.push(report);
        }
    }

    Ok(reports)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepomdXml {
    #[serde(rename = "#content")]
    pub data: Vec<RepomdXmlItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepomdXmlItem {
    #[serde(rename = "@type")]
    pub item_type: Option<String>,
    pub location: Option<RepomdXmlLocation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepomdXmlLocation {
    #[serde(rename = "@href")]
    pub href: String,
}

fn get_primary_location_from_xml(bytes: &[u8]) -> Result<String> {
    let list = serde_xml_rs::from_reader::<RepomdXml, _>(bytes)?;
    let primary = list
        .data
        .into_iter()
        .find(|x| x.item_type.as_deref() == Some("primary"))
        .context("Failed to find 'primary' in repomd file")?;
    let location = primary
        .location
        .context("Failed to find 'location' attribute")?;
    Ok(location.href)
}

fn parse_package_index<R: Read>(r: R) -> Result<Vec<PackagesXmlItem>> {
    let list = serde_xml_rs::from_reader::<PackagesXml, _>(r)?;
    Ok(list.packages)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackagesXml {
    #[serde(rename = "#content")]
    pub packages: Vec<PackagesXmlItem>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PackagesXmlItem {
    pub name: String,
    pub arch: String,
    pub version: PackagesXmlItemVersion,
    pub packager: String,
    pub location: PackagesXmlItemLocation,
    pub format: PackagesXmlItemMetadata,
}

impl Pkg for PackagesXmlItem {
    fn pkg_name(&self) -> &str {
        &self.name
    }

    fn by_maintainer(&self, maintainers: &[String]) -> bool {
        maintainers.iter().any(|m| self.packager.starts_with(m))
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PackagesXmlItemVersion {
    #[serde(rename = "@epoch")]
    pub epoch: String,
    #[serde(rename = "@ver")]
    pub ver: String,
    #[serde(rename = "@rel")]
    pub rel: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PackagesXmlItemLocation {
    #[serde(rename = "@href")]
    pub href: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PackagesXmlItemMetadata {
    #[serde(rename = "rpm:sourcerpm")]
    pub sourcerpm: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repomd_primary_location() -> Result<()> {
        let bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<repomd xmlns="http://linux.duke.edu/metadata/repo" xmlns:rpm="http://linux.duke.edu/metadata/rpm">
  <revision>1681418230</revision>
  <data type="primary">
    <checksum type="sha256">f6dee453a7f86804214e402ad2e444b989f044f0b16fa7ba74e5a27a8a49cd07</checksum>
    <open-checksum type="sha256">7f2fdf1ef2d90b5a7223cee806e2cd1652b6086b3f747a3986352b48d4be638f</open-checksum>
    <location href="repodata/f6dee453a7f86804214e402ad2e444b989f044f0b16fa7ba74e5a27a8a49cd07-primary.xml.gz"/>
    <timestamp>1681418141</timestamp>
    <size>18320782</size>
    <open-size>167638461</open-size>
  </data>
  <data type="filelists">
    <checksum type="sha256">131fa4fcd206fd3a718e4765983c8b7b276e7e634e45c226d9c465145f8e69e9</checksum>
    <open-checksum type="sha256">f02c846e6937f434f4800d58c9ff11f5b42076ea2081c9d94e0ad9f4ea949c06</open-checksum>
    <location href="repodata/131fa4fcd206fd3a718e4765983c8b7b276e7e634e45c226d9c465145f8e69e9-filelists.xml.gz"/>
    <timestamp>1681418141</timestamp>
    <size>54340868</size>
    <open-size>747611414</open-size>
  </data>
  <data type="other">
    <checksum type="sha256">1c4bf077a2bdf4743a7cded3e2f72282dec5f8e4910692d193e371508552322a</checksum>
    <open-checksum type="sha256">8bce951885e14c3bcd16563a84077afa954d5ba39149e9b2ae7755fc433538d8</open-checksum>
    <location href="repodata/1c4bf077a2bdf4743a7cded3e2f72282dec5f8e4910692d193e371508552322a-other.xml.gz"/>
    <timestamp>1681418141</timestamp>
    <size>7374868</size>
    <open-size>110707385</open-size>
  </data>
  <data type="primary_db">
    <checksum type="sha256">60bf45195ec1a08fb8f269632660d98db897920ad3444478c4a2ce48bac6f8c5</checksum>
    <open-checksum type="sha256">1e721ade786dee7c1fd34ca44386a1879c192618c9353c90a89a5a694c909846</open-checksum>
    <location href="repodata/60bf45195ec1a08fb8f269632660d98db897920ad3444478c4a2ce48bac6f8c5-primary.sqlite.xz"/>
    <timestamp>1681418198</timestamp>
    <size>28710824</size>
    <open-size>169746432</open-size>
    <database_version>10</database_version>
  </data>
  <data type="filelists_db">
    <checksum type="sha256">da0f6cd3c50c54b4a0d1de4156483a84d952400ca6a1a9f18215e2f8153de53a</checksum>
    <open-checksum type="sha256">307d77854cb35d06937ab7c096e180bb2e00131a488b64f8b90a2994a10d65d1</open-checksum>
    <location href="repodata/da0f6cd3c50c54b4a0d1de4156483a84d952400ca6a1a9f18215e2f8153de53a-filelists.sqlite.xz"/>
    <timestamp>1681418227</timestamp>
    <size>45336776</size>
    <open-size>347639808</open-size>
    <database_version>10</database_version>
  </data>
  <data type="other_db">
    <checksum type="sha256">e0dee9e0e57a903c855ac065ee2ecf213352136c5f0b84f693ed313ab6900569</checksum>
    <open-checksum type="sha256">c3f167024efc8f66d199bd836ae49155eed857659d6811939316689f805f7136</open-checksum>
    <location href="repodata/e0dee9e0e57a903c855ac065ee2ecf213352136c5f0b84f693ed313ab6900569-other.sqlite.xz"/>
    <timestamp>1681418164</timestamp>
    <size>10661728</size>
    <open-size>94199808</open-size>
    <database_version>10</database_version>
  </data>
  <data type="primary_zck">
    <checksum type="sha256">dfa2aec8d2e83459677d698542f1a190d92815888668d027b21dfd46fb86ce01</checksum>
    <open-checksum type="sha256">7f2fdf1ef2d90b5a7223cee806e2cd1652b6086b3f747a3986352b48d4be638f</open-checksum>
    <header-checksum type="sha256">34d4d0da333f4e5935ee08100f5fe9dd78de016444839e1cfc8776aeb527fbc2</header-checksum>
    <location href="repodata/dfa2aec8d2e83459677d698542f1a190d92815888668d027b21dfd46fb86ce01-primary.xml.zck"/>
    <timestamp>1681418141</timestamp>
    <size>33215921</size>
    <open-size>167638461</open-size>
    <header-size>541899</header-size>
  </data>
  <data type="filelists_zck">
    <checksum type="sha256">5f86dfe1903316d6b25d494616cf18352204fe70c529b4c97859df6faecd493f</checksum>
    <open-checksum type="sha256">f02c846e6937f434f4800d58c9ff11f5b42076ea2081c9d94e0ad9f4ea949c06</open-checksum>
    <header-checksum type="sha256">73a46a4969de23c71eb8bbc364bfa7b1380672fb26828de14723783dcc6745ea</header-checksum>
    <location href="repodata/5f86dfe1903316d6b25d494616cf18352204fe70c529b4c97859df6faecd493f-filelists.xml.zck"/>
    <timestamp>1681418141</timestamp>
    <size>51796916</size>
    <open-size>747611414</open-size>
    <header-size>545530</header-size>
  </data>
  <data type="other_zck">
    <checksum type="sha256">dfe295aec5a168406ec81db7a5d1e4f8df75cfb9fa11be9ed2a8b17d8f9ced18</checksum>
    <open-checksum type="sha256">8bce951885e14c3bcd16563a84077afa954d5ba39149e9b2ae7755fc433538d8</open-checksum>
    <header-checksum type="sha256">ea63e740b565e42a5eafda8514c1b1e20d18ff00a12869539fdfd292f72c05f6</header-checksum>
    <location href="repodata/dfe295aec5a168406ec81db7a5d1e4f8df75cfb9fa11be9ed2a8b17d8f9ced18-other.xml.zck"/>
    <timestamp>1681418141</timestamp>
    <size>15491666</size>
    <open-size>110707385</open-size>
    <header-size>541197</header-size>
  </data>
  <data type="group">
    <checksum type="sha256">1ab074f803c33a54eac36ffee514cfe9a60a927416b3ddc34defeaf4b1d63776</checksum>
    <location href="repodata/1ab074f803c33a54eac36ffee514cfe9a60a927416b3ddc34defeaf4b1d63776-comps-Everything.x86_64.xml"/>
    <timestamp>1681417845</timestamp>
    <size>1826355</size>
  </data>
  <data type="group_xz">
    <checksum type="sha256">5a8d7109590bf585b77781785ca3cfc4ad835e23700fdf5c28b256486c3182a1</checksum>
    <location href="repodata/5a8d7109590bf585b77781785ca3cfc4ad835e23700fdf5c28b256486c3182a1-comps-Everything.x86_64.xml.xz"/>
    <timestamp>1681418141</timestamp>
    <size>265548</size>
  </data>
  <data type="group_zck">
    <checksum type="sha256">49e47bfcdc5d63156c36b4ab0028c39def5002bcef03cb6e024f12a0b3200df7</checksum>
    <open-checksum type="sha256">5a8d7109590bf585b77781785ca3cfc4ad835e23700fdf5c28b256486c3182a1</open-checksum>
    <header-checksum type="sha256">928cb71330c7e6cc99138d5cb8c6cd5fd3bd07a0d479bf49421883c9f6d1f1f9</header-checksum>
    <location href="repodata/49e47bfcdc5d63156c36b4ab0028c39def5002bcef03cb6e024f12a0b3200df7-comps-Everything.x86_64.xml.zck"/>
    <timestamp>1681418230</timestamp>
    <size>496815</size>
    <open-size>265548</open-size>
    <header-size>1203</header-size>
  </data>
</repomd>
"#;
        let location = get_primary_location_from_xml(bytes)?;
        assert_eq!(location, "repodata/f6dee453a7f86804214e402ad2e444b989f044f0b16fa7ba74e5a27a8a49cd07-primary.xml.gz");
        Ok(())
    }

    #[test]
    fn test_parse_repomd_package_list() -> Result<()> {
        let bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<metadata xmlns="http://linux.duke.edu/metadata/common" xmlns:rpm="http://linux.duke.edu/metadata/rpm" packages="69222">
<package type="rpm">
  <name>0ad</name>
  <arch>x86_64</arch>
  <version epoch="0" ver="0.0.26" rel="7.fc38"/>
  <checksum type="sha256" pkgid="YES">6df9f2af65c505b47d42bd8183091e6c04b9a13290712937b3fdbc67c46f9e95</checksum>
  <summary>Cross-Platform RTS Game of Ancient Warfare</summary>
  <description>0 A.D. (pronounced "zero ey-dee") is a free, open-source, cross-platform
real-time strategy (RTS) game of ancient warfare. In short, it is a
historically-based war/economy game that allows players to relive or rewrite
the history of Western civilizations, focusing on the years between 500 B.C.
and 500 A.D. The project is highly ambitious, involving state-of-the-art 3D
graphics, detailed artwork, sound, and a flexible and powerful custom-built
game engine.

The game has been in development by Wildfire Games (WFG), a group of volunteer,
hobbyist game developers, since 2001.</description>
  <packager>Fedora Project</packager>
  <url>http://play0ad.com</url>
  <time file="1672456075" build="1672453415"/>
  <size package="9140732" installed="30315589" archive="30350532"/>
  <location href="Packages/0/0ad-0.0.26-7.fc38.x86_64.rpm"/>
  <format>
    <rpm:license>GPLv2+ and BSD and MIT and IBM and MPLv2.0</rpm:license>
    <rpm:vendor>Fedora Project</rpm:vendor>
    <rpm:group>Unspecified</rpm:group>
    <rpm:buildhost>buildvm-x86-16.iad2.fedoraproject.org</rpm:buildhost>
    <rpm:sourcerpm>0ad-0.0.26-7.fc38.src.rpm</rpm:sourcerpm>
    <rpm:header-range start="29752" end="67893"/>
    <rpm:provides>
      <rpm:entry name="0ad" flags="EQ" epoch="0" ver="0.0.26" rel="7.fc38"/>
      <rpm:entry name="0ad(x86-64)" flags="EQ" epoch="0" ver="0.0.26" rel="7.fc38"/>
      <rpm:entry name="application()"/>
      <rpm:entry name="application(0ad.desktop)"/>
      <rpm:entry name="bundled(mozjs)" flags="EQ" epoch="0" ver="78"/>
      <rpm:entry name="metainfo()"/>
      <rpm:entry name="metainfo(0ad.appdata.xml)"/>
      <rpm:entry name="mimehandler(application/x-pyromod+zip)"/>
    </rpm:provides>
    <rpm:requires>
      <rpm:entry name="/usr/bin/sh"/>
      <rpm:entry name="0ad-data" flags="EQ" epoch="0" ver="0.0.26"/>
      <rpm:entry name="hicolor-icon-theme"/>
      <rpm:entry name="ld-linux-x86-64.so.2()(64bit)"/>
      <rpm:entry name="ld-linux-x86-64.so.2(GLIBC_2.3)(64bit)"/>
      <rpm:entry name="libSDL2-2.0.so.0()(64bit)"/>
      <rpm:entry name="libX11.so.6()(64bit)"/>
      <rpm:entry name="libboost_filesystem.so.1.78.0()(64bit)"/>
      <rpm:entry name="libcurl.so.4()(64bit)"/>
      <rpm:entry name="libenet.so.7()(64bit)"/>
      <rpm:entry name="libfmt.so.9()(64bit)"/>
      <rpm:entry name="libfreetype.so.6()(64bit)"/>
      <rpm:entry name="libgcc_s.so.1()(64bit)"/>
      <rpm:entry name="libgcc_s.so.1(GCC_3.0)(64bit)"/>
      <rpm:entry name="libgcc_s.so.1(GCC_3.3)(64bit)"/>
      <rpm:entry name="libgcc_s.so.1(GCC_3.4)(64bit)"/>
      <rpm:entry name="libgcc_s.so.1(GCC_4.2.0)(64bit)"/>
      <rpm:entry name="libgloox.so.17()(64bit)"/>
      <rpm:entry name="libicui18n.so.72()(64bit)"/>
      <rpm:entry name="libicuuc.so.72()(64bit)"/>
      <rpm:entry name="libm.so.6()(64bit)"/>
      <rpm:entry name="libm.so.6(GLIBC_2.2.5)(64bit)"/>
      <rpm:entry name="libm.so.6(GLIBC_2.27)(64bit)"/>
      <rpm:entry name="libm.so.6(GLIBC_2.29)(64bit)"/>
      <rpm:entry name="libminiupnpc.so.17()(64bit)"/>
      <rpm:entry name="libnvtt.so.2.1()(64bit)"/>
      <rpm:entry name="libopenal.so.1()(64bit)"/>
      <rpm:entry name="libpng16.so.16()(64bit)"/>
      <rpm:entry name="libpng16.so.16(PNG16_0)(64bit)"/>
      <rpm:entry name="libsodium.so.23()(64bit)"/>
      <rpm:entry name="libstdc++.so.6()(64bit)"/>
      <rpm:entry name="libstdc++.so.6(CXXABI_1.3)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(CXXABI_1.3.5)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(CXXABI_1.3.7)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(CXXABI_1.3.8)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(CXXABI_1.3.9)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.11)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.14)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.15)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.17)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.18)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.19)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.20)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.21)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.22)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.26)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.29)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.30)(64bit)"/>
      <rpm:entry name="libstdc++.so.6(GLIBCXX_3.4.9)(64bit)"/>
      <rpm:entry name="libvorbisfile.so.3()(64bit)"/>
      <rpm:entry name="libwx_baseu-3.2.so.0()(64bit)"/>
      <rpm:entry name="libwx_baseu-3.2.so.0(WXU_3.2)(64bit)"/>
      <rpm:entry name="libwx_baseu_xml-3.2.so.0()(64bit)"/>
      <rpm:entry name="libwx_baseu_xml-3.2.so.0(WXU_3.2)(64bit)"/>
      <rpm:entry name="libwx_gtk3u_core-3.2.so.0()(64bit)"/>
      <rpm:entry name="libwx_gtk3u_core-3.2.so.0(WXU_3.2)(64bit)"/>
      <rpm:entry name="libwx_gtk3u_gl-3.2.so.0()(64bit)"/>
      <rpm:entry name="libwx_gtk3u_gl-3.2.so.0(WXU_3.2)(64bit)"/>
      <rpm:entry name="libxml2.so.2()(64bit)"/>
      <rpm:entry name="libxml2.so.2(LIBXML2_2.4.30)(64bit)"/>
      <rpm:entry name="libxml2.so.2(LIBXML2_2.5.2)(64bit)"/>
      <rpm:entry name="libxml2.so.2(LIBXML2_2.6.0)(64bit)"/>
      <rpm:entry name="libxml2.so.2(LIBXML2_2.6.21)(64bit)"/>
      <rpm:entry name="libxml2.so.2(LIBXML2_2.9.0)(64bit)"/>
      <rpm:entry name="libz.so.1()(64bit)"/>
      <rpm:entry name="libz.so.1(ZLIB_1.2.0)(64bit)"/>
      <rpm:entry name="rtld(GNU_HASH)"/>
      <rpm:entry name="libc.so.6(GLIBC_2.34)(64bit)"/>
    </rpm:requires>
    <file>/usr/bin/0ad</file>
    <file>/usr/bin/pyrogenesis</file>
  </format>
</package>
<package type="rpm">
  <name>0ad-data</name>
  <arch>noarch</arch>
  <version epoch="0" ver="0.0.26" rel="2.fc38"/>
  <checksum type="sha256" pkgid="YES">9d4882481909c8c5cdd4b59988f17fc015d8703f5fbc587d07bd44038fbdb9ac</checksum>
  <summary>The Data Files for 0 AD</summary>
  <description>0 A.D. (pronounced "zero ey-dee") is a free, open-source, cross-platform
real-time strategy (RTS) game of ancient warfare. In short, it is a
historically-based war/economy game that allows players to relive or rewrite
the history of Western civilizations, focusing on the years between 500 B.C.
and 500 A.D. The project is highly ambitious, involving state-of-the-art 3D
graphics, detailed artwork, sound, and a flexible and powerful custom-built
game engine.

This package contains the 0ad data files.</description>
  <packager>Fedora Project</packager>
  <url>http://play0ad.com</url>
  <time file="1674076145" build="1674071971"/>
  <size package="1493523190" installed="3296032344" archive="3296040572"/>
  <location href="Packages/0/0ad-data-0.0.26-2.fc38.noarch.rpm"/>
  <format>
    <rpm:license>CC-BY-SA</rpm:license>
    <rpm:vendor>Fedora Project</rpm:vendor>
    <rpm:group>Unspecified</rpm:group>
    <rpm:buildhost>buildvm-a64-28.iad2.fedoraproject.org</rpm:buildhost>
    <rpm:sourcerpm>0ad-data-0.0.26-2.fc38.src.rpm</rpm:sourcerpm>
    <rpm:header-range start="11984" end="22489"/>
    <rpm:provides>
      <rpm:entry name="0ad-data" flags="EQ" epoch="0" ver="0.0.26" rel="2.fc38"/>
    </rpm:provides>
    <rpm:requires>
      <rpm:entry name="dejavu-sans-fonts"/>
      <rpm:entry name="dejavu-sans-mono-fonts"/>
    </rpm:requires>
  </format>
</package>
<package type="rpm">
  <name>0xFFFF</name>
  <arch>x86_64</arch>
  <version epoch="0" ver="0.10" rel="2.fc38"/>
  <checksum type="sha256" pkgid="YES">77fbc5a6edd3091c45c171e84081a20a755498e865ccab3726bf3e52c5a82733</checksum>
  <summary>The Open Free Fiasco Firmware Flasher</summary>
  <description>The 'Open Free Fiasco Firmware Flasher' aka 0xFFFF utility implements
a free (GPL3) userspace handler for the NOLO bootloader and related
utilities for the Nokia Internet Tablets like flashing setting device
options, packing/unpacking FIASCO firmware format and more.</description>
  <packager>Fedora Project</packager>
  <url>https://talk.maemo.org/showthread.php?t=87996</url>
  <time file="1674073729" build="1674070719"/>
  <size package="77271" installed="191439" archive="192992"/>
  <location href="Packages/0/0xFFFF-0.10-2.fc38.x86_64.rpm"/>
  <format>
    <rpm:license>GPLv3</rpm:license>
    <rpm:vendor>Fedora Project</rpm:vendor>
    <rpm:group>Unspecified</rpm:group>
    <rpm:buildhost>buildvm-x86-07.iad2.fedoraproject.org</rpm:buildhost>
    <rpm:sourcerpm>0xFFFF-0.10-2.fc38.src.rpm</rpm:sourcerpm>
    <rpm:header-range start="4504" end="9757"/>
    <rpm:provides>
      <rpm:entry name="0xFFFF" flags="EQ" epoch="0" ver="0.10" rel="2.fc38"/>
      <rpm:entry name="0xFFFF(x86-64)" flags="EQ" epoch="0" ver="0.10" rel="2.fc38"/>
    </rpm:provides>
    <rpm:requires>
      <rpm:entry name="libusb-0.1.so.4()(64bit)"/>
      <rpm:entry name="rtld(GNU_HASH)"/>
      <rpm:entry name="libc.so.6(GLIBC_2.34)(64bit)"/>
    </rpm:requires>
    <file>/usr/bin/0xFFFF</file>
  </format>
</package>
</metadata>
"#;
        let list = parse_package_index(&bytes[..])?;
        assert_eq!(
            list,
            &[
                PackagesXmlItem {
                    name: "0ad".to_string(),
                    arch: "x86_64".to_string(),
                    version: PackagesXmlItemVersion {
                        epoch: "0".to_string(),
                        ver: "0.0.26".to_string(),
                        rel: "7.fc38".to_string(),
                    },
                    packager: "Fedora Project".to_string(),
                    location: PackagesXmlItemLocation {
                        href: "Packages/0/0ad-0.0.26-7.fc38.x86_64.rpm".to_string(),
                    },
                    format: PackagesXmlItemMetadata {
                        sourcerpm: "0ad-0.0.26-7.fc38.src.rpm".to_string(),
                    }
                },
                PackagesXmlItem {
                    name: "0ad-data".to_string(),
                    arch: "noarch".to_string(),
                    version: PackagesXmlItemVersion {
                        epoch: "0".to_string(),
                        ver: "0.0.26".to_string(),
                        rel: "2.fc38".to_string(),
                    },
                    packager: "Fedora Project".to_string(),
                    location: PackagesXmlItemLocation {
                        href: "Packages/0/0ad-data-0.0.26-2.fc38.noarch.rpm".to_string(),
                    },
                    format: PackagesXmlItemMetadata {
                        sourcerpm: "0ad-data-0.0.26-2.fc38.src.rpm".to_string(),
                    }
                },
                PackagesXmlItem {
                    name: "0xFFFF".to_string(),
                    arch: "x86_64".to_string(),
                    version: PackagesXmlItemVersion {
                        epoch: "0".to_string(),
                        ver: "0.10".to_string(),
                        rel: "2.fc38".to_string(),
                    },
                    packager: "Fedora Project".to_string(),
                    location: PackagesXmlItemLocation {
                        href: "Packages/0/0xFFFF-0.10-2.fc38.x86_64.rpm".to_string(),
                    },
                    format: PackagesXmlItemMetadata {
                        sourcerpm: "0xFFFF-0.10-2.fc38.src.rpm".to_string(),
                    }
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn test_parse_repomd_package_list_item() -> Result<()> {
        let bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<metadata xmlns="http://linux.duke.edu/metadata/common" xmlns:rpm="http://linux.duke.edu/metadata/rpm" packages="69222">
  <name>0ad-data</name>
  <arch>noarch</arch>
  <version epoch="0" ver="0.0.26" rel="2.fc38"/>
  <checksum type="sha256" pkgid="YES">9d4882481909c8c5cdd4b59988f17fc015d8703f5fbc587d07bd44038fbdb9ac</checksum>
  <summary>The Data Files for 0 AD</summary>
  <description>0 A.D. (pronounced "zero ey-dee") is a free, open-source, cross-platform
real-time strategy (RTS) game of ancient warfare. In short, it is a
historically-based war/economy game that allows players to relive or rewrite
the history of Western civilizations, focusing on the years between 500 B.C.
and 500 A.D. The project is highly ambitious, involving state-of-the-art 3D
graphics, detailed artwork, sound, and a flexible and powerful custom-built
game engine.

This package contains the 0ad data files.</description>
  <packager>Fedora Project</packager>
  <url>http://play0ad.com</url>
  <time file="1674076145" build="1674071971"/>
  <size package="1493523190" installed="3296032344" archive="3296040572"/>
  <location href="Packages/0/0ad-data-0.0.26-2.fc38.noarch.rpm"/>
  <format>
    <rpm:license>CC-BY-SA</rpm:license>
    <rpm:vendor>Fedora Project</rpm:vendor>
    <rpm:group>Unspecified</rpm:group>
    <rpm:buildhost>buildvm-a64-28.iad2.fedoraproject.org</rpm:buildhost>
    <rpm:sourcerpm>0ad-data-0.0.26-2.fc38.src.rpm</rpm:sourcerpm>
    <rpm:header-range start="11984" end="22489"/>
    <rpm:provides>
      <rpm:entry name="0ad-data" flags="EQ" epoch="0" ver="0.0.26" rel="2.fc38"/>
    </rpm:provides>
    <rpm:requires>
      <rpm:entry name="dejavu-sans-fonts"/>
      <rpm:entry name="dejavu-sans-mono-fonts"/>
    </rpm:requires>
  </format>
</metadata>
"#;
        let pkg = serde_xml_rs::from_reader::<PackagesXmlItem, _>(&bytes[..])?;
        assert_eq!(
            pkg,
            PackagesXmlItem {
                name: "0ad-data".to_string(),
                arch: "noarch".to_string(),
                version: PackagesXmlItemVersion {
                    epoch: "0".to_string(),
                    ver: "0.0.26".to_string(),
                    rel: "2.fc38".to_string(),
                },
                packager: "Fedora Project".to_string(),
                location: PackagesXmlItemLocation {
                    href: "Packages/0/0ad-data-0.0.26-2.fc38.noarch.rpm".to_string(),
                },
                format: PackagesXmlItemMetadata {
                    sourcerpm: "0ad-data-0.0.26-2.fc38.src.rpm".to_string(),
                }
            },
        );
        Ok(())
    }
}
