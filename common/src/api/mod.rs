use crate::auth;
use crate::config::ConfigFile;
use crate::errors::Error;
use crate::utils::zstd_compress;
use anyhow::{Context, anyhow};
use async_trait::async_trait;
use log::debug;
use reqwest::header::CONTENT_ENCODING;
use reqwest::{RequestBuilder, Response};
use std::borrow::Cow;
use std::env;
use url::Url;

pub mod v0;
pub mod v1;

pub const AUTH_COOKIE_HEADER: &str = "X-Auth-Cookie";
pub const WORKER_KEY_HEADER: &str = "X-Worker-Key";
pub const SIGNUP_SECRET_HEADER: &str = "X-Signup-Secret";

pub struct Client {
    endpoint: Url,
    client: crate::http::Client,
    is_default_endpoint: bool,
    auth_cookie: Option<String>,
    worker_key: Option<String>,
    signup_secret: Option<String>,
}

impl Client {
    pub fn new(config: ConfigFile, endpoint: Option<String>) -> anyhow::Result<Client> {
        let (endpoint, auth_cookie, is_default_endpoint) = if let Some(endpoint) = endpoint {
            let cookie = config
                .endpoints
                .get(&endpoint)
                .map(|e| e.cookie.to_string());
            (endpoint, cookie, false)
        } else if let Some(endpoint) = config.http.endpoint {
            (endpoint, None, true)
        } else {
            ("http://127.0.0.1:8484".to_string(), None, true)
        };

        let mut endpoint = endpoint
            .parse::<Url>()
            .with_context(|| anyhow!("Failed to parse endpoint as url: {:?}", endpoint))?;

        // If the url ends with a slash, remove it
        endpoint
            .path_segments_mut()
            .map_err(|_| anyhow!("Given endpoint url cannot be base"))?
            .pop_if_empty();

        debug!("Setting rebuilderd endpoint to {:?}", endpoint.as_str());
        let client = crate::http::Client::builder().zstd(true).build()?;

        Ok(Client {
            endpoint,
            client,
            is_default_endpoint,
            auth_cookie,
            worker_key: None,
            signup_secret: None,
        })
    }

    pub fn with_auth_cookie(&mut self) -> anyhow::Result<&mut Self> {
        if let Ok(cookie_path) = env::var("REBUILDERD_COOKIE_PATH") {
            debug!("Found cookie path in environment: {:?}", cookie_path);
            let auth_cookie =
                auth::read_cookie_from_file(cookie_path).context("Failed to load auth cookie")?;
            Ok(self.auth_cookie(auth_cookie))
        } else if self.is_default_endpoint {
            let auth_cookie = auth::find_auth_cookie().context("Failed to load auth cookie")?;
            Ok(self.auth_cookie(auth_cookie))
        } else {
            Ok(self)
        }
    }

    pub fn auth_cookie<I: Into<String>>(&mut self, cookie: I) -> &mut Self {
        self.auth_cookie = Some(cookie.into());
        self
    }

    pub fn worker_key<I: Into<String>>(&mut self, key: I) {
        self.worker_key = Some(key.into());
    }

    pub fn signup_secret<I: Into<String>>(&mut self, secret: I) {
        self.signup_secret = Some(secret.into());
    }

    fn url_join(&self, route: &str) -> Url {
        let mut url = self.endpoint.clone();
        {
            // this unwrap is safe because we've called path_segments_mut in the constructor before
            let mut path = url.path_segments_mut().expect("Url cannot be base");
            for segment in route.split('/') {
                path.push(segment);
            }
        }

        url
    }

    fn authenticated(&self, mut req: RequestBuilder) -> RequestBuilder {
        if let Some(auth_cookie) = &self.auth_cookie {
            req = req.header(AUTH_COOKIE_HEADER, auth_cookie);
        }

        if let Some(worker_key) = &self.worker_key {
            req = req.header(WORKER_KEY_HEADER, worker_key);
        }

        if let Some(signup_secret) = &self.signup_secret {
            req = req.header(SIGNUP_SECRET_HEADER, signup_secret);
        }

        req
    }

    fn get(&self, path: Cow<'static, str>) -> crate::http::RequestBuilder {
        let url = self.url_join(&path);
        debug!("Sending GET request to {}", url.as_str());
        let req = self.client.get(url);
        self.authenticated(req)
    }

    fn post(&self, path: Cow<'static, str>) -> crate::http::RequestBuilder {
        let url = self.url_join(&path);
        debug!("Sending POST request to {}", url.as_str());
        let req = self.client.post(url);
        self.authenticated(req)
    }

    fn delete(&self, path: Cow<'static, str>) -> crate::http::RequestBuilder {
        let url = self.url_join(&path);
        debug!("Sending DELETE request to {}", url.as_str());
        let req = self.client.delete(url);
        self.authenticated(req)
    }
}

#[async_trait]
pub trait ZstdRequestBuilder {
    async fn send_encoded(self) -> crate::errors::Result<Response>;
}

#[async_trait]
impl ZstdRequestBuilder for RequestBuilder {
    async fn send_encoded(self) -> crate::errors::Result<Response> {
        if let Some(new_request) = self.try_clone() {
            let mut request = self.build()?;

            if let Some(body) = request.body_mut() {
                if let Some(bytes) = body.as_bytes() {
                    let encoded_body = zstd_compress(bytes).await?;

                    new_request
                        .body(encoded_body)
                        .header(CONTENT_ENCODING, "zstd")
                        .send()
                        .await
                        .map_err(Error::from)
                } else {
                    new_request.send().await.map_err(Error::from)
                }
            } else {
                new_request.send().await.map_err(Error::from)
            }
        } else {
            self.send().await.map_err(Error::from)
        }
    }
}
