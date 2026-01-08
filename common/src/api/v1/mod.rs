mod models;

use crate::api::{Client, ZstdRequestBuilder};
use crate::errors::*;
use async_trait::async_trait;
pub use models::*;
use std::borrow::Cow;

#[cfg(feature = "diesel")]
use diesel::{
    deserialize::FromSql,
    serialize::{IsNull, Output, ToSql},
    sql_types::Integer,
    sqlite::{Sqlite, SqliteValue},
    {AsExpression, FromSqlRow},
};
use serde::{Deserialize, Serialize};

/// Represents the priority of an enqueued rebuild job. The job queue is sorted based on priority and
/// time, so the lower this number is, the more prioritized the job is. It's a little backwards, but
/// hey.
///
/// There are some utility functions on the type for accessing default values for well-defined use
/// cases. These map to constants in the same namespace as this type, and you can use either one.
/// ```
/// use rebuilderd_common::api::v1::Priority;
///
/// assert_eq!(Priority::from(1), Priority::default());
/// assert_eq!(Priority::from(2), Priority::retry());
/// assert_eq!(Priority::from(0), Priority::manual());
/// ```
///
/// You can also set a completely custom priority. This is mostly useful for external API calls that
/// orchestrate rebuilds.
/// ```
/// use rebuilderd_common::api::v1::Priority;
///
/// let custom = Priority::from(10);
/// assert_eq!(custom, Priority::from(10));
///
/// ```
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
#[cfg_attr(feature = "diesel", derive(FromSqlRow, AsExpression))]
#[cfg_attr(feature = "diesel", diesel(sql_type = Integer))]
#[cfg_attr(feature = "diesel", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
pub struct Priority(i32);

impl Priority {
    /// The default priority for enqueued rebuilds. The job queue is sorted based on priority and time,
    /// so the lower this number is, the more prioritized the job is. It's a little backwards, but hey.
    const DEFAULT_QUEUE_PRIORITY: i32 = 1;

    /// The default priority used for automatically requeued jobs. This priority is lower than the one
    /// for untested packages.
    const DEFAULT_RETRY_PRIORITY: i32 = Self::DEFAULT_QUEUE_PRIORITY + 1;

    /// The default priority used for manually retried jobs. This priority is higher than the one for
    /// untested packages.
    const DEFAULT_MANUAL_PRIORITY: i32 = Self::DEFAULT_QUEUE_PRIORITY - 1;

    pub fn retry() -> Self {
        Priority(Self::DEFAULT_RETRY_PRIORITY)
    }

    pub fn manual() -> Self {
        Priority(Self::DEFAULT_MANUAL_PRIORITY)
    }
}

impl Default for Priority {
    fn default() -> Self {
        Priority(Self::DEFAULT_QUEUE_PRIORITY)
    }
}

#[cfg(feature = "diesel")]
impl FromSql<Integer, Sqlite> for Priority {
    fn from_sql(bytes: SqliteValue) -> diesel::deserialize::Result<Self> {
        let value = <i32 as FromSql<Integer, Sqlite>>::from_sql(bytes)?;
        Ok(Priority(value))
    }
}

#[cfg(feature = "diesel")]
impl ToSql<Integer, Sqlite> for Priority {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> diesel::serialize::Result {
        out.set_value(self.0);
        Ok(IsNull::No)
    }
}

impl From<i32> for Priority {
    fn from(value: i32) -> Self {
        Priority(value)
    }
}

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
    async fn get_worker_tags(&self, id: i32) -> Result<Vec<String>>;
    async fn set_worker_tags(&self, id: i32, tags: Vec<String>) -> Result<()>;
    async fn create_worker_tag(&self, id: i32, tag: String) -> Result<()>;
    async fn delete_worker_tag(&self, id: i32, tag: String) -> Result<()>;
}

#[async_trait]
pub trait TagRestApi {
    async fn get_tags(&self) -> Result<Vec<String>>;
    async fn create_tag(&self, request: CreateTagRequest) -> Result<String>;
    async fn delete_tag(&self, tag: String) -> Result<()>;
    async fn get_tag_rules(&self, tag: String) -> Result<Vec<TagRule>>;
    async fn create_tag_rule(&self, tag: String, request: CreateTagRuleRequest) -> Result<TagRule>;
    async fn delete_tag_rule(&self, tag: String, tag_rule_id: i32) -> Result<()>;
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
            .get(Cow::Owned(format!("api/v1/builds/{id}/log")))
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
        // nginx dies if proxying a request without a Content-Length header
        self.post(Cow::Owned(format!("api/v1/queue/{id}/ping")))
            .header("Content-Length", 0)
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

    async fn get_worker_tags(&self, id: i32) -> Result<Vec<String>> {
        let tags = self
            .get(Cow::Owned(format!("api/v1/workers/{id}/tags")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(tags)
    }

    async fn set_worker_tags(&self, id: i32, tags: Vec<String>) -> Result<()> {
        self.put(Cow::Owned(format!("api/v1/workers/{id}/tags")))
            .json(&tags)
            .send_encoded()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn create_worker_tag(&self, id: i32, tag: String) -> Result<()> {
        self.put(Cow::Owned(format!("api/v1/workers/{id}/tags/{tag}")))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn delete_worker_tag(&self, id: i32, tag: String) -> Result<()> {
        self.delete(Cow::Owned(format!("api/v1/workers/{id}/tags/{tag}")))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[async_trait]
impl TagRestApi for Client {
    async fn get_tags(&self) -> Result<Vec<String>> {
        let records = self
            .get(Cow::Borrowed("api/v1/tags"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(records)
    }

    async fn create_tag(&self, request: CreateTagRequest) -> Result<String> {
        let record = self
            .post(Cow::Borrowed("api/v1/tags"))
            .json(&request)
            .send_encoded()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(record)
    }

    async fn delete_tag(&self, tag: String) -> Result<()> {
        self.delete(Cow::Owned(format!("api/v1/tags/{tag}")))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn get_tag_rules(&self, tag: String) -> Result<Vec<TagRule>> {
        let records = self
            .get(Cow::Owned(format!("api/v1/tags/{tag}")))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(records)
    }

    async fn create_tag_rule(&self, tag: String, request: CreateTagRuleRequest) -> Result<TagRule> {
        let record = self
            .post(Cow::Owned(format!("api/v1/tags/{tag}")))
            .json(&request)
            .send_encoded()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(record)
    }

    async fn delete_tag_rule(&self, tag: String, tag_rule_id: i32) -> Result<()> {
        self.delete(Cow::Owned(format!("api/v1/tags/{tag}/{tag_rule_id}")))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}
