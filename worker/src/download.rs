use futures_util::StreamExt;
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use url::Url;

pub async fn download(url_str: &str, path: &Path) -> Result<PathBuf> {
    let url = url_str
        .parse::<Url>()
        .context("Failed to parse input as url")?;

    let filename = url
        .path_segments()
        .ok_or_else(|| format_err!("Url doesn't seem to have a path"))?
        .next_back()
        .ok_or_else(|| format_err!("Failed to get filename from path"))?
        .to_owned();
    if filename.is_empty() {
        bail!("Filename detected from url is empty");
    }

    let target = path.join(&filename);

    info!("Downloading {:?} to {:?}", url_str, target);
    let client = http::client()?;
    let mut stream = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes_stream();

    let mut f = File::create(&target)
        .await
        .context("Failed to create output file")?;

    let mut bytes = 0;
    while let Some(item) = stream.next().await {
        let item = item?;
        f.write_all(&item).await?;
        bytes += item.len();
    }
    info!("Downloaded {} bytes", bytes);

    Ok(PathBuf::from(filename))
}
