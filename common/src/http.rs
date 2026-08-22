use crate::errors::*;
pub use reqwest::{Client, RequestBuilder};
use std::time::Duration;

pub fn client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .user_agent(concat!("rebuilderd/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(Error::from)
}
