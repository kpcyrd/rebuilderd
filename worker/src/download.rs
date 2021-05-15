use futures_util::StreamExt;
use rebuilderd_common::errors::*;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use url::Url;

pub async fn download(url: &str, path: &Path) -> Result<(String, String)> {
    let url = url.parse::<Url>()
        .context("Failed to parse input as url")?;

    let filename = url.path_segments()
        .ok_or_else(|| format_err!("Url doesn't seem to have a path"))?
        .last()
        .ok_or_else(|| format_err!("Failed to get filename from path"))?;
    if filename.is_empty() {
        bail!("Filename is empty");
    }

    let target = path.join(filename);

    info!("Downloading {:?} to {:?}", url, target);
    let client = reqwest::Client::new();
    let mut stream = client.get(&url.to_string())
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

    let target = target.to_str()
        .ok_or_else(|| format_err!("Input path contains invalid characters"))?;

    Ok((target.to_string(), filename.to_string()))
}
