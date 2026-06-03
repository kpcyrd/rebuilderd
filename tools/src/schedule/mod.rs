use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use std::fs;

pub mod archlinux;
pub mod debian;
pub mod fedora;
pub mod tails;

pub async fn fetch_url_or_path(client: &http::Client, path: &str) -> Result<Vec<u8>> {
    let bytes = if path.starts_with("https://") || path.starts_with("http://") {
        info!("Downloading {:?}...", path);
        client
            .get(path)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
            .to_vec()
    } else {
        info!("Reading {:?}...", path);
        fs::read(path)?
    };

    Ok(bytes)
}

pub trait Pkg {
    fn binary_pkg_name(&self) -> &str;

    fn source_pkg_name(&self) -> Option<&str>;

    fn maintainers(&self) -> Box<dyn Iterator<Item = &str> + '_>;
}
