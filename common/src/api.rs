use chrono::prelude::*;
use crate::config::ConfigFile;
use crate::errors::*;
use crate::{PkgRelease, PkgGroup, Status};
use crate::auth;
use reqwest::{Client as HttpClient, RequestBuilder};
use serde::{Serialize, Deserialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use url::Url;

pub const AUTH_COOKIE_HEADER: &str = "X-Auth-Cookie";
pub const WORKER_KEY_HEADER: &str = "X-Worker-Key";
pub const SIGNUP_SECRET_HEADER: &str = "X-Signup-Secret";

pub struct Client {
    endpoint: Url,
    client: HttpClient,
    is_default_endpoint: bool,
    auth_cookie: Option<String>,
    worker_key: Option<String>,
    signup_secret: Option<String>,
}

impl Client {
    pub fn new(config: ConfigFile, endpoint: Option<String>) -> Result<Client> {
        let (endpoint, auth_cookie, is_default_endpoint) = if let Some(endpoint) = endpoint {
            let cookie = config.endpoints.get(&endpoint)
                .map(|e| e.cookie.to_string());
            (endpoint, cookie, false)
        } else if let Some(endpoint) = config.http.endpoint {
            (endpoint, None, true)
        } else {
            ("http://127.0.0.1:8484".to_string(), None, true)
        };

        let mut endpoint = endpoint.parse::<Url>()
            .with_context(|| anyhow!("Failed to parse endpoint as url: {:?}", endpoint))?;

        // If the url ends with a slash, remove it
        endpoint.path_segments_mut().map_err(|_| anyhow!("Given endpoint url cannot be base"))?
            .pop_if_empty();

        debug!("Setting rebuilderd endpoint to {:?}", endpoint.as_str());
        let client = HttpClient::new();
        Ok(Client {
            endpoint,
            client,
            is_default_endpoint,
            auth_cookie,
            worker_key: None,
            signup_secret: None,
        })
    }

    pub fn with_auth_cookie(&mut self) -> Result<&mut Self> {
        if let Ok(cookie_path) = env::var("REBUILDERD_COOKIE_PATH") {
            debug!("Found cookie path in environment: {:?}", cookie_path);
            let auth_cookie = auth::read_cookie_from_file(cookie_path)
                .context("Failed to load auth cookie")?;
            Ok(self.auth_cookie(auth_cookie))
        } else if self.is_default_endpoint {
            let auth_cookie = auth::find_auth_cookie()
                .context("Failed to load auth cookie")?;
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

    pub fn get(&self, path: Cow<'static,str>) -> RequestBuilder {
        let mut req = self.client.get(self.url_join(&path));
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

    pub fn post(&self, path: Cow<'static, str>) -> RequestBuilder {
        let mut req = self.client.post(self.url_join(&path));
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

    pub async fn list_workers(&self) -> Result<Vec<Worker>> {
        let workers = self.get(Cow::Borrowed("api/v0/workers"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(workers)
    }

    pub async fn sync_suite(&self, import: &SuiteImport) -> Result<()> {
        self.post(Cow::Borrowed("api/v0/pkgs/sync"))
            .json(import)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn list_pkgs(&self, list: &ListPkgs) -> Result<Vec<PkgRelease>> {
        let pkgs = self.get(Cow::Borrowed("api/v0/pkgs/list"))
            .query(list)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pkgs)
    }

    pub async fn match_one_pkg(&self, list: &ListPkgs) -> Result<PkgRelease> {
        let pkgs = self.list_pkgs(list).await?;

        if pkgs.len() > 1 {
            bail!("Filter matched too many packages: {}", pkgs.len());
        }

        let pkg = pkgs.into_iter()
            .next()
            .context("Filter didn't match any packages on this rebuilder")?;

        Ok(pkg)
    }

    pub async fn fetch_log(&self, id: i32) -> Result<Vec<u8>> {
        let log = self.get(Cow::Owned(format!("api/v0/builds/{}/log", id)))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        Ok(log.to_vec())
    }

    pub async fn fetch_diffoscope(&self, id: i32) -> Result<Vec<u8>> {
        let log = self.get(Cow::Owned(format!("api/v0/builds/{}/diffoscope", id)))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        Ok(log.to_vec())
    }

    pub async fn fetch_attestation(&self, id: i32) -> Result<Vec<u8>> {
        let attestation = self.get(Cow::Owned(format!("api/v0/builds/{}/attestation", id)))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        Ok(attestation.to_vec())
    }

    pub async fn list_queue(&self, list: &ListQueue) -> Result<QueueList> {
        let pkgs = self.post(Cow::Borrowed("api/v0/queue/list"))
            .json(list)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pkgs)
    }

    pub async fn push_queue(&self, push: &PushQueue) -> Result<()> {
        self.post(Cow::Borrowed("api/v0/queue/push"))
            .json(push)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(())
    }

    pub async fn pop_queue(&self, query: &WorkQuery) -> Result<JobAssignment> {
        let assignment = self.post(Cow::Borrowed("api/v0/queue/pop"))
            .json(query)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(assignment)
    }

    pub async fn drop_queue(&self, query: &DropQueueItem) -> Result<()> {
        self.post(Cow::Borrowed("api/v0/queue/drop"))
            .json(query)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(())
    }

    pub async fn requeue_pkgs(&self, requeue: &RequeueQuery) -> Result<()> {
        self.post(Cow::Borrowed("api/v0/pkg/requeue"))
            .json(requeue)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(())
    }

    pub async fn ping_build(&self, ticket: &QueueItem) -> Result<()> {
        self.post(Cow::Borrowed("api/v0/build/ping"))
            .json(ticket)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn report_build(&self, ticket: &BuildReport) -> Result<()> {
        self.post(Cow::Borrowed("api/v0/build/report"))
            .json(ticket)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Success {
    Ok,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Worker {
    pub key: String,
    pub addr: String,
    pub status: Option<String>,
    pub last_ping: NaiveDateTime,
    pub online: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkQuery {
    pub supported_backends: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum JobAssignment {
    Nothing,
    Rebuild(Box<QueueItem>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuiteImport {
    pub distro: String,
    pub suite: String,
    pub groups: Vec<PkgGroup>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListPkgs {
    pub name: Option<String>,
    pub status: Option<Status>,
    pub distro: Option<String>,
    pub suite: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueList {
    pub now: NaiveDateTime,
    pub queue: Vec<QueueItem>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: i32,
    pub pkgbase: PkgGroup,
    pub version: String,
    pub queued_at: NaiveDateTime,
    pub worker_id: Option<i32>,
    pub started_at: Option<NaiveDateTime>,
    pub last_ping: Option<NaiveDateTime>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListQueue {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushQueue {
    pub name: String,
    pub version: Option<String>,
    pub priority: i32,
    pub distro: String,
    pub suite: String,
    pub architecture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DropQueueItem {
    pub name: String,
    pub version: Option<String>,
    pub distro: String,
    pub suite: String,
    pub architecture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequeueQuery {
    pub name: Option<String>,
    pub status: Option<Status>,
    pub priority: i32,
    pub distro: Option<String>,
    pub suite: Option<String>,
    pub architecture: Option<String>,
    pub reset: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum BuildStatus {
    Good,
    Bad,
    Fail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rebuild {
    pub status: BuildStatus,
    pub log: String,
    pub diffoscope: Option<String>,
    pub attestation: Option<String>,
}

impl Rebuild {
    pub fn new(status: BuildStatus, log: String) -> Rebuild {
        Rebuild {
            status,
            log,
            diffoscope: None,
            attestation: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildReport {
    pub queue: QueueItem,
    pub rebuilds: Vec<Rebuild>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardResponse {
    pub suites: HashMap<String, SuiteStats>,
    pub active_builds: Vec<QueueItem>,
    pub queue_length: usize,
    pub now: NaiveDateTime,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SuiteStats {
    pub good: usize,
    pub unknown: usize,
    pub bad: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_format_default() {
        let client = Client::new(ConfigFile::default(), None).unwrap();
        assert_eq!(client.endpoint, "http://127.0.0.1:8484".parse().unwrap());
    }

    #[test]
    fn test_endpoint_format_example_com() {
        let client = Client::new(ConfigFile::default(), Some("https://example.com".into())).unwrap();
        assert_eq!(client.endpoint, "https://example.com".parse().unwrap());
    }

    #[test]
    fn test_endpoint_format_example_com_trailing_slash() {
        let client = Client::new(ConfigFile::default(), Some("https://example.com/".into())).unwrap();
        assert_eq!(client.endpoint, "https://example.com".parse().unwrap());
    }

    #[test]
    fn test_endpoint_format_example_com_with_path() {
        let client = Client::new(ConfigFile::default(), Some("https://example.com/re/build".into())).unwrap();
        assert_eq!(client.endpoint, "https://example.com/re/build".parse().unwrap());
    }

    #[test]
    fn test_endpoint_format_example_com_with_path_trailing_slash() {
        let client = Client::new(ConfigFile::default(), Some("https://example.com/re/build/".into())).unwrap();
        assert_eq!(client.endpoint, "https://example.com/re/build".parse().unwrap());
    }
}
