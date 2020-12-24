use chrono::prelude::*;
use crate::config::ConfigFile;
use crate::errors::*;
use crate::{Distro, PkgRelease, PkgGroup, Status};
use crate::auth;
use reqwest::{Client as HttpClient, RequestBuilder};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use tokio_compat_02::FutureExt;

pub const AUTH_COOKIE_HEADER: &str = "X-Auth-Cookie";
pub const WORKER_KEY_HEADER: &str = "X-Worker-Key";
pub const SIGNUP_SECRET_HEADER: &str = "X-Signup-Secret";

pub struct Client {
    endpoint: String,
    client: HttpClient,
    is_default_endpoint: bool,
    auth_cookie: Option<String>,
    worker_key: Option<String>,
    signup_secret: Option<String>,
}

impl Client {
    pub fn new(config: ConfigFile, endpoint: Option<String>) -> Client {
        let (endpoint, auth_cookie, is_default_endpoint) = if let Some(endpoint) = endpoint {
            let cookie = config.endpoints.get(&endpoint)
                .map(|e| e.cookie.to_string());
            (endpoint, cookie, false)
        } else if let Some(endpoint) = config.http.endpoint {
            (endpoint, None, true)
        } else {
            ("http://127.0.0.1:8484".to_string(), None, true)
        };

        debug!("setting rebuilderd endpoint to {:?}", endpoint);
        let client = HttpClient::new();
        Client {
            endpoint,
            client,
            is_default_endpoint,
            auth_cookie,
            worker_key: None,
            signup_secret: None,
        }
    }

    pub fn with_auth_cookie(&mut self) -> Result<&mut Self> {
        if self.is_default_endpoint {
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

    pub fn get(&self, path: &'static str) -> RequestBuilder {
        let mut req = self.client.get(&format!("{}{}", self.endpoint, path));
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

    pub fn post(&self, path: &'static str) -> RequestBuilder {
        let mut req = self.client.post(&format!("{}{}", self.endpoint, path));
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
        let workers = self.get("/api/v0/workers")
            .send()
            .compat()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(workers)
    }

    pub async fn sync_suite(&self, import: &SuiteImport) -> Result<()> {
        self.post("/api/v0/pkgs/sync")
            .json(import)
            .send()
            .compat()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn list_pkgs(&self, list: &ListPkgs) -> Result<Vec<PkgRelease>> {
        let pkgs = self.get("/api/v0/pkgs/list")
            .query(list)
            .send()
            .compat()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pkgs)
    }

    pub async fn list_queue(&self, list: &ListQueue) -> Result<QueueList> {
        let pkgs = self.post("/api/v0/queue/list")
            .json(list)
            .send()
            .compat()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pkgs)
    }

    pub async fn push_queue(&self, push: &PushQueue) -> Result<()> {
        self.post("/api/v0/queue/push")
            .json(push)
            .send()
            .compat()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(())
    }

    pub async fn pop_queue(&self, query: &WorkQuery) -> Result<JobAssignment> {
        let assignment = self.post("/api/v0/queue/pop")
            .json(query)
            .send()
            .compat()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(assignment)
    }

    pub async fn drop_queue(&self, query: &DropQueueItem) -> Result<()> {
        self.post("/api/v0/queue/drop")
            .json(query)
            .send()
            .compat()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(())
    }

    pub async fn requeue_pkgs(&self, requeue: &RequeueQuery) -> Result<()> {
        self.post("/api/v0/pkg/requeue")
            .json(requeue)
            .send()
            .compat()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(())
    }

    pub async fn ping_build(&self, ticket: &QueueItem) -> Result<()> {
        self.post("/api/v0/build/ping")
            .json(ticket)
            .send()
            .compat()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn report_build(&self, ticket: &BuildReport) -> Result<()> {
        self.post("/api/v0/build/report")
            .json(ticket)
            .send()
            .compat()
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
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum JobAssignment {
    Nothing,
    Rebuild(QueueItem),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuiteImport {
    pub distro: Distro,
    pub suite: String,
    pub architecture: String,
    pub pkgs: Vec<PkgGroup>,
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
    pub package: PkgRelease,
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
}

impl Rebuild {
    pub fn new(status: BuildStatus, log: String) -> Rebuild {
        Rebuild {
            status,
            log,
            diffoscope: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildReport {
    pub queue: QueueItem,
    pub rebuild: Rebuild,
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
