use crate::errors::*;
pub use reqwest::{Client, RequestBuilder};
use std::time::Duration;

pub fn client() -> Result<Client> {
    Client::builder()
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(Error::from)
}
