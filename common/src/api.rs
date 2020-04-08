use crate::errors::*;
use chrono::prelude::*;
use serde::{Serialize, Deserialize};
use crate::{Distro, PkgRelease};
use reqwest::RequestBuilder;

pub const WORKER_HEADER: &str = "X-Worker-Key";

pub struct Client {
    endpoint: String,
    client: reqwest::Client,
    worker_key: Option<String>,
}

impl Client {
    pub fn new(endpoint: String) -> Client {
        let client = reqwest::Client::new();
        Client {
            endpoint,
            client,
            worker_key: None,
        }
    }

    pub fn worker_key<I: Into<String>>(&mut self, key: I) {
        self.worker_key = Some(key.into());
    }

    pub fn get(&self, path: &'static str) -> RequestBuilder {
        let mut req = self.client.get(&format!("{}{}", self.endpoint, path));
        if let Some(worker_key) = &self.worker_key {
            req = req.header(WORKER_HEADER, worker_key);
        }
        req
    }

    pub fn post(&self, path: &'static str) -> RequestBuilder {
        let mut req = self.client.post(&format!("{}{}", self.endpoint, path));
        if let Some(worker_key) = &self.worker_key {
            req = req.header(WORKER_HEADER, worker_key);
        }
        req
    }

    pub async fn list_workers(&self) -> Result<Vec<Worker>> {
        let workers = self.get("/api/v0/workers")
            .send()
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
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn list_pkgs(&self, list: &ListPkgs) -> Result<Vec<PkgRelease>> {
        let pkgs = self.get("/api/v0/pkgs/list")
            .query(list)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pkgs)
    }

    pub async fn list_queue(&self, list: &ListQueue) -> Result<Vec<QueueItem>> {
        let pkgs = self.post("/api/v0/queue/list")
            .json(list)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pkgs)
    }

    pub async fn pop_queue(&self, query: &WorkQuery) -> Result<JobAssignment> {
        let assignment = self.post("/api/v0/queue/pop")
            .json(query)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(assignment)
    }

    pub async fn ping_build(&self, ticket: &QueueItem) -> Result<()> {
        self.post("/api/v0/build/ping")
            .json(ticket)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn report_build(&self, ticket: &BuildReport) -> Result<()> {
        self.post("/api/v0/build/report")
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
}

#[derive(Debug, Serialize, Deserialize)]
pub enum JobAssignment {
    Nothing,
    Rebuild(QueueItem),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuiteImport {
    pub distro: Distro,
    pub suite: String,
    pub architecture: String,
    pub pkgs: Vec<PkgRelease>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListPkgs {
    pub name: Option<String>,
    pub status: Option<String>,
    pub distro: Option<String>,
    pub suite: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: i32,
    pub package: PkgRelease,
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
pub enum BuildStatus {
    Good,
    Bad,
    Fail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildReport {
    pub queue: QueueItem,
    pub status: BuildStatus,
}
