use crate::args::PkgsSync;
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use rebuilderd_common::{PkgArtifact, PkgGroup};
use regex::Regex;
use url::Url;

pub async fn sync(http: &http::Client, sync: &PkgsSync) -> Result<Vec<PkgGroup>> {
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

    info!("Detecting tails versions");

    let re = Regex::new(r"tails-amd64-([0-9a-z~\.]+)/").unwrap();
    let cap = re
        .captures_iter(&directory_list)
        .next()
        .ok_or_else(|| anyhow!("Regular expression didn't match any versions"))?;
    let version = &cap[1];

    info!("Detected tails version: {:?}", version);

    let mut group = PkgGroup::new(
        "tails".to_string(),
        version.to_string(),
        "tails".to_string(),
        sync.suite.to_string(),
        "amd64".to_string(),
        None,
    );

    for ext in &["img", "iso"] {
        let filename = format!("tails-amd64-{}.{}", version, ext);

        let mut url = source.clone();
        url.path_segments_mut()
            .map_err(|_| anyhow!("cannot be base"))?
            .pop_if_empty()
            .extend(&[&sync.suite, &format!("tails-amd64-{}", version), &filename]);

        let url = url.to_string();
        info!("Artifact url: {:?}", url);

        let artifact = PkgArtifact {
            name: filename,
            version: version.to_string(),
            url,
        };
        group.add_artifact(artifact);
    }

    Ok(vec![group])
}
