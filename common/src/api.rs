use crate::errors::*;
use chrono::prelude::*;
use serde::{Serialize, Deserialize};
use crate::{Distro, PkgRelease};

pub struct Client {
    endpoint: &'static str,
    client: reqwest::Client,
}

impl Client {
    pub fn new() -> Client {
        let client = reqwest::Client::new();
        Client {
            endpoint: "http://127.0.0.1:8080",
            client,
        }
    }

    pub async fn list_workers(&self) -> Result<Vec<Worker>> {
        let workers = self.client.get(&format!("{}/api/v0/workers", self.endpoint))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(workers)
    }

    pub async fn sync_suite(&self, import: &SuiteImport) -> Result<()> {
        self.client.post(&format!("{}/api/v0/pkgs/sync", self.endpoint))
            .json(import)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn list_pkgs(&self, list: &ListPkgs) -> Result<Vec<PkgRelease>> {
        let pkgs = self.client.get(&format!("{}/api/v0/pkgs/list", self.endpoint))
            .query(list)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pkgs)
    }

    pub async fn list_queue(&self, list: &ListQueue) -> Result<Vec<QueueItem>> {
        let pkgs = self.client.post(&format!("{}/api/v0/queue/list", self.endpoint))
            .json(list)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pkgs)
    }

    pub async fn pop_queue(&self, query: &WorkQuery) -> Result<JobAssignment> {
        let assignment = self.client.post(&format!("{}/api/v0/queue/pop", self.endpoint))
            .json(query)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(assignment)
    }

    pub async fn ping_build(&self, ticket: &BuildTicket) -> Result<()> {
        self.client.post(&format!("{}/api/v0/build/ping", self.endpoint))
            .json(ticket)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn report_build(&self, ticket: &BuildReport) -> Result<()> {
        self.client.post(&format!("{}/api/v0/build/report", self.endpoint))
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
    pub key: String,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildTicket {
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BuildStatus {
    Good,
    Bad,
    Fail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildReport {
    pub pkg: PkgRelease,
    pub status: BuildStatus,
}
