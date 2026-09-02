use crate::args::PkgsSync;
use crate::rules;
use crate::schedule::repomd;
use rebuilderd_common::api::v1::{BinaryPackageReport, PackageReport, SourcePackageReport};
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use std::collections::BTreeMap;

pub async fn sync(http: &http::Client, sync: &PkgsSync) -> Result<Vec<PackageReport>> {
    let mut reports = Vec::new();

    for release in &sync.releases {
        for arch in &sync.architectures {
            let mut report = PackageReport {
                distribution: "fedora".to_string(),
                release: None,
                architecture: arch.clone(),
                packages: Vec::new(),
            };

            let mut bases: BTreeMap<_, SourcePackageReport> = BTreeMap::new();

            for component in &sync.components {
                let base_url = format!(
                    "{}/{}/{}/{}/os",
                    release.source(&sync.source),
                    release.name(),
                    component,
                    arch
                );
                let packages = repomd::fetch_package_index(http, &base_url).await?;

                for pkg in packages {
                    if !rules::matches(sync, &pkg, component) {
                        continue;
                    }

                    let url = format!("{base_url}/{}", pkg.location.href);
                    let version = format!("{}-{}", pkg.version.ver, pkg.version.rel);
                    let artifact = BinaryPackageReport {
                        name: pkg.name,
                        version,
                        component: Some(component.clone()),
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
            }

            report.packages = bases.into_values().collect();
            reports.push(report);
        }
    }

    Ok(reports)
}
