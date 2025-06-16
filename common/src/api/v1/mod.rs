mod models;

use crate::api::{Client, ZstdRequestBuilder};
use crate::errors::*;
use async_trait::async_trait;
pub use models::*;
use std::borrow::Cow;

#[async_trait]
pub trait BuildRestApi {
    async fn get_builds(
        &self,
        page: Option<&Page>,
        origin_filter: Option<&OriginFilter>,
        identity_filter: Option<&IdentityFilter>,
    ) -> Result<ResultPage<Rebuild>>;

    async fn submit_build_report(&self, request: RebuildReport) -> Result<()>;
    async fn get_build(&self, id: i32) -> Result<Rebuild>;
    async fn get_build_log(&self, id: i32) -> Result<String>;
    async fn get_build_artifacts(&self, id: i32) -> Result<Vec<RebuildArtifact>>;
    async fn get_build_artifact(&self, id: i32, artifact_id: i32) -> Result<RebuildArtifact>;
    async fn get_build_artifact_diffoscope(&self, id: i32, artifact_id: i32) -> Result<String>;
    async fn get_build_artifact_attestation(&self, id: i32, artifact_id: i32) -> Result<Vec<u8>>;
}

#[async_trait]
pub trait DashboardRestApi {
    async fn get_dashboard(&self, origin_filter: Option<&OriginFilter>) -> Result<OriginFilter>;
}

#[async_trait]
pub trait MetaRestApi {
    async fn get_distributions(&self) -> Result<Vec<String>>;
    async fn get_distribution_releases(&self, distribution: &str) -> Result<Vec<String>>;
    async fn get_distribution_architectures(&self, distribution: &str) -> Result<Vec<String>>;
    async fn get_distribution_components(&self, distribution: &str) -> Result<Vec<String>>;
    async fn get_distribution_release_architectures(
        &self,
        distribution: &str,
        release: &str,
    ) -> Result<Vec<String>>;

    async fn get_distribution_release_components(
        &self,
        distribution: &str,
        release: &str,
    ) -> Result<Vec<String>>;

    async fn get_distribution_release_component_architectures(
        &self,
        distribution: &str,
        release: &str,
        component: &str,
    ) -> Result<Vec<String>>;

    async fn get_public_keys(&self) -> Result<PublicKey>;
}

#[async_trait]
pub trait PackageRestApi {
    async fn submit_package_report(&self, report: &PackageReport) -> Result<()>;

    async fn get_source_packages(
        &self,
        page: Option<&Page>,
        origin_filter: Option<&OriginFilter>,
        identity_filter: Option<&IdentityFilter>,
    ) -> Result<ResultPage<SourcePackage>>;

    async fn get_source_package(&self, id: i32) -> Result<SourcePackage>;

    async fn get_binary_packages(
        &self,
        page: Option<&Page>,
        origin_filter: Option<&OriginFilter>,
        identity_filter: Option<&IdentityFilter>,
    ) -> Result<ResultPage<BinaryPackage>>;

    async fn get_binary_package(&self, id: i32) -> Result<BinaryPackage>;
}

#[async_trait]
pub trait QueueRestApi {
    async fn get_queued_jobs(
        &self,
        page: Option<&Page>,
        origin_filter: Option<&OriginFilter>,
        identity_filter: Option<&IdentityFilter>,
    ) -> Result<ResultPage<QueuedJob>>;

    async fn request_rebuild(&self, request: QueueJobRequest) -> Result<()>;
    async fn get_queued_job(&self, id: i32) -> Result<QueuedJob>;
    async fn drop_queued_job(&self, id: i32) -> Result<()>;
    async fn drop_queued_jobs(
        &self,
        origin_filter: Option<&OriginFilter>,
        identity_filter: Option<&IdentityFilter>,
    ) -> Result<()>;
    async fn request_work(&self, request: PopQueuedJobRequest) -> Result<JobAssignment>;
    async fn ping_job(&self, id: i32) -> Result<()>;
}

#[async_trait]
pub trait WorkerRestApi {
    async fn get_workers(&self, page: Option<&Page>) -> Result<ResultPage<Worker>>;
    async fn register_worker(&self, request: RegisterWorkerRequest) -> Result<()>;
    async fn get_worker(&self, id: i32) -> Result<Worker>;
    async fn unregister_worker(&self, id: i32) -> Result<()>;
}

#[async_trait]
impl BuildRestApi for Client {
    async fn get_builds(
        &self,
        page: Option<&Page>,
        origin_filter: Option<&OriginFilter>,
        identity_filter: Option<&IdentityFilter>,
    ) -> Result<ResultPage<Rebuild>> {
        let records = self
            .get(Cow::Borrowed("api/v1/builds"))
            .query(&page)
            .query(&origin_filter)
            .query(&identity_filter)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(records)
    }

    async fn submit_build_report(&self, request: RebuildReport) -> Result<()> {
        self.post(Cow::Borrowed("api/v1/builds"))
            .json(&request)
            .send_encoded()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn get_build(&self, id: i32) -> Result<Rebuild> {
        let record = self
            .get(Cow::Owned(format!("api/v1/builds/{id}")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(record)
    }

    async fn get_build_log(&self, id: i32) -> Result<String> {
        let data = self
            .get(Cow::Owned(format!("api/v1/packages/source/{id}")))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(data)
    }

    async fn get_build_artifacts(&self, id: i32) -> Result<Vec<RebuildArtifact>> {
        let records = self
            .get(Cow::Owned(format!("api/v1/builds/{id}/artifacts")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(records)
    }

    async fn get_build_artifact(&self, id: i32, artifact_id: i32) -> Result<RebuildArtifact> {
        let record = self
            .get(Cow::Owned(format!(
                "api/v1/builds/{id}/artifacts/{artifact_id}"
            )))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(record)
    }

    async fn get_build_artifact_diffoscope(&self, id: i32, artifact_id: i32) -> Result<String> {
        let data = self
            .get(Cow::Owned(format!(
                "api/v1/builds/{id}/artifacts/{artifact_id}/diffoscope"
            )))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(data)
    }

    async fn get_build_artifact_attestation(&self, id: i32, artifact_id: i32) -> Result<Vec<u8>> {
        let data = self
            .get(Cow::Owned(format!(
                "api/v1/builds/{id}/artifacts/{artifact_id}/attestation"
            )))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        Ok(Vec::from(data))
    }
}

#[async_trait]
impl DashboardRestApi for Client {
    async fn get_dashboard(&self, origin_filter: Option<&OriginFilter>) -> Result<OriginFilter> {
        let dashboard = self
            .get(Cow::Borrowed("api/v1/dashboard"))
            .query(&origin_filter)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(dashboard)
    }
}

#[async_trait]
impl MetaRestApi for Client {
    async fn get_distributions(&self) -> Result<Vec<String>> {
        let results = self
            .get(Cow::Borrowed("api/v1/meta/distributions"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(results)
    }

    async fn get_distribution_releases(&self, distribution: &str) -> Result<Vec<String>> {
        let results = self
            .get(Cow::Owned(format!(
                "api/v1/meta/distributions/{distribution}/releases"
            )))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(results)
    }

    async fn get_distribution_architectures(&self, distribution: &str) -> Result<Vec<String>> {
        let results = self
            .get(Cow::Owned(format!(
                "api/v1/meta/distributions/{distribution}/architectures"
            )))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(results)
    }

    async fn get_distribution_components(&self, distribution: &str) -> Result<Vec<String>> {
        let results = self
            .get(Cow::Owned(format!(
                "api/v1/meta/distributions/{distribution}/components"
            )))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(results)
    }

    async fn get_distribution_release_architectures(
        &self,
        distribution: &str,
        release: &str,
    ) -> Result<Vec<String>> {
        let results = self
            .get(Cow::Owned(format!(
                "api/v1/meta/distributions/{distribution}/releases/{release}/architectures"
            )))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(results)
    }

    async fn get_distribution_release_components(
        &self,
        distribution: &str,
        release: &str,
    ) -> Result<Vec<String>> {
        let results = self
            .get(Cow::Owned(format!(
                "api/v1/meta/distributions/{distribution}/releases/{release}/components"
            )))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(results)
    }

    async fn get_distribution_release_component_architectures(
        &self,
        distribution: &str,
        release: &str,
        component: &str,
    ) -> Result<Vec<String>> {
        let results = self
            .get(Cow::Owned(format!(
                "api/v1/meta/distributions/{distribution}/releases/{release}/components/{component}/architectures"
            )))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(results)
    }

    async fn get_public_keys(&self) -> Result<PublicKey> {
        let public_key = self
            .get(Cow::Borrowed("api/v1/meta/public-keys"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(public_key)
    }
}

#[async_trait]
impl PackageRestApi for Client {
    async fn submit_package_report(&self, report: &PackageReport) -> Result<()> {
        self.post(Cow::Borrowed("api/v1/packages"))
            .json(report)
            .send_encoded()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn get_source_packages(
        &self,
        page: Option<&Page>,
        origin_filter: Option<&OriginFilter>,
        identity_filter: Option<&IdentityFilter>,
    ) -> Result<ResultPage<SourcePackage>> {
        let records = self
            .get(Cow::Borrowed("api/v1/packages/source"))
            .query(&page)
            .query(&origin_filter)
            .query(&identity_filter)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(records)
    }

    async fn get_source_package(&self, id: i32) -> Result<SourcePackage> {
        let record = self
            .get(Cow::Owned(format!("api/v1/packages/source/{id}")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(record)
    }

    async fn get_binary_packages(
        &self,
        page: Option<&Page>,
        origin_filter: Option<&OriginFilter>,
        identity_filter: Option<&IdentityFilter>,
    ) -> Result<ResultPage<BinaryPackage>> {
        let records = self
            .get(Cow::Borrowed("api/v1/packages/binary"))
            .query(&page)
            .query(&origin_filter)
            .query(&identity_filter)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(records)
    }

    async fn get_binary_package(&self, id: i32) -> Result<BinaryPackage> {
        let record = self
            .get(Cow::Owned(format!("api/v1/packages/binary/{id}")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(record)
    }
}

#[async_trait]
impl QueueRestApi for Client {
    async fn get_queued_jobs(
        &self,
        page: Option<&Page>,
        origin_filter: Option<&OriginFilter>,
        identity_filter: Option<&IdentityFilter>,
    ) -> Result<ResultPage<QueuedJob>> {
        let records = self
            .get(Cow::Borrowed("api/v1/queue"))
            .query(&page)
            .query(&origin_filter)
            .query(&identity_filter)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(records)
    }

    async fn request_rebuild(&self, request: QueueJobRequest) -> Result<()> {
        self.post(Cow::Borrowed("api/v1/queue"))
            .json(&request)
            .send_encoded()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn get_queued_job(&self, id: i32) -> Result<QueuedJob> {
        let record = self
            .get(Cow::Owned(format!("api/v1/queue/{id}")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(record)
    }

    async fn drop_queued_job(&self, id: i32) -> Result<()> {
        self.delete(Cow::Owned(format!("api/v1/queue/{id}")))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn drop_queued_jobs(
        &self,
        origin_filter: Option<&OriginFilter>,
        identity_filter: Option<&IdentityFilter>,
    ) -> Result<()> {
        self.delete(Cow::Borrowed("api/v1/queue"))
            .query(&origin_filter)
            .query(&identity_filter)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn request_work(&self, request: PopQueuedJobRequest) -> Result<JobAssignment> {
        let record = self
            .post(Cow::Borrowed("api/v1/queue/pop"))
            .json(&request)
            .send_encoded()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(record)
    }

    async fn ping_job(&self, id: i32) -> Result<()> {
        self.post(Cow::Owned(format!("api/v1/queue/{id}/ping")))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[async_trait]
impl WorkerRestApi for Client {
    async fn get_workers(&self, page: Option<&Page>) -> Result<ResultPage<Worker>> {
        let workers = self
            .get(Cow::Borrowed("api/v1/workers"))
            .query(&page)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(workers)
    }

    async fn register_worker(&self, request: RegisterWorkerRequest) -> Result<()> {
        self.post(Cow::Borrowed("api/v1/workers"))
            .json(&request)
            .send_encoded()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn get_worker(&self, id: i32) -> Result<Worker> {
        let worker = self
            .get(Cow::Owned(format!("api/v1/workers/{id}")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(worker)
    }

    async fn unregister_worker(&self, id: i32) -> Result<()> {
        self.delete(Cow::Owned(format!("api/v1/workers/{id}")))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}
