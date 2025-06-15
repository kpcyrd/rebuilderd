use crate::args::PkgsSync;
use rebuilderd_common::api::v1::{BinaryPackageReport, PackageReport, SourcePackageReport};
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use regex::Regex;
use url::Url;

pub async fn sync(http: &http::Client, sync: &PkgsSync) -> Result<Vec<PackageReport>> {
    let source = sync
        .source
        .parse::<Url>()
        .context("Failed to parse source as url")?;

    let mut url = source.clone();
    url.path_segments_mut()
        .map_err(|_| anyhow!("cannot be base"))?
        .pop_if_empty()
        .push(&sync.suite);

    info!("Downloading directory list from {}", url);
    let directory_list = http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let mut reports = Vec::new();
    for release in &sync.releases {
        for architecture in &sync.architectures {
            let mut report = PackageReport {
                distribution: "tails".to_string(),
                release: Some(release.clone()),
                component: None,
                architecture: architecture.clone(),
                packages: Vec::new(),
            };

            info!("Detecting tails versions");

            let prefix = format!("tails-{architecture}");
            let re = Regex::new(&*(prefix + r"-([0-9a-z~\.]+)/")).unwrap();
            let cap = re
                .captures_iter(&directory_list)
                .next()
                .ok_or_else(|| anyhow!("Regular expression didn't match any versions"))?;
            let version = &cap[1];

            info!("Detected tails version: {:?}", version);

            let mut group: Option<SourcePackageReport> = None;

            for ext in &["img", "iso"] {
                let filename = format!("tails-{architecture}-{version}.{ext}");

                let mut url = source.clone();
                url.path_segments_mut()
                    .map_err(|_| anyhow!("cannot be base"))?
                    .pop_if_empty()
                    .extend(&[
                        &sync.suite,
                        &format!("tails-{architecture}-{version}"),
                        &filename,
                    ]);

                let url = url.to_string();
                info!("Artifact url: {:?}", url);

                let artifact = BinaryPackageReport {
                    name: filename,
                    version: version.to_string(),
                    architecture: architecture.clone(),
                    url: url.clone(),
                };

                if let Some(ref mut group) = group {
                    group.artifacts.push(artifact);
                } else {
                    let new_group = SourcePackageReport {
                        name: "tails".to_string(),
                        version: version.to_string(),
                        url: url.clone(), // use first artifact's url as the source URL for now
                        artifacts: vec![artifact],
                    };

                    group = Some(new_group);
                }
            }

            report.packages = vec![group.unwrap()];
            reports.push(report);
        }
    }

    Ok(reports)
}
