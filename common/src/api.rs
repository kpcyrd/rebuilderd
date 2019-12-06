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

    pub async fn get_work(&self, query: &WorkQuery) -> Result<JobAssignment> {
        let assignment = self.client.post(&format!("{}/api/v0/job/fetch", self.endpoint))
            .json(query)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(assignment)
    }

    pub async fn sync_suite(&self, import: &SuiteImport) -> Result<()> {
        self.client.post(&format!("{}/api/v0/job/sync", self.endpoint))
            .json(import)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn list_pkgs(&self, list: &ListPkgs) -> Result<Vec<PkgRelease>> {
        let pkgs = self.client.post(&format!("{}/api/v0/pkgs/list", self.endpoint))
            .json(list)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pkgs)
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
    pub distro: Option<String>,
    pub suite: Option<String>,
    pub architecture: Option<String>,
}
