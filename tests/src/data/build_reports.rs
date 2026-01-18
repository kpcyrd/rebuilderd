use crate::data::{create_dummy_signed_attestation, create_dummy_unsigned_attestation};
use chrono::Utc;
use rebuilderd_common::api::v1::{
    ArtifactStatus, BuildStatus, QueuedJobWithArtifacts, RebuildArtifactReport, RebuildReport,
};
use rebuilderd_common::utils::zstd_compress;

pub const DUMMY_BUILD_LOG: &str = "build-log";
pub const DUMMY_DIFFOSCOPE: &str = "diffoscope";

pub fn bad_rebuild_report(job: &QueuedJobWithArtifacts) -> RebuildReport {
    let mut artifacts = Vec::new();
    for artifact in job.artifacts.clone() {
        artifacts.push(RebuildArtifactReport {
            name: artifact.name.clone(),
            diffoscope: Some(DUMMY_DIFFOSCOPE.to_string().into_bytes()),
            status: ArtifactStatus::Bad,
            attestation: None,
        });
    }

    RebuildReport {
        queue_id: job.job.id,
        built_at: Utc::now().naive_utc(),
        build_log: DUMMY_BUILD_LOG.to_string().into_bytes(),
        status: BuildStatus::Bad,
        artifacts,
    }
}

pub fn failed_rebuild_report(job: &QueuedJobWithArtifacts) -> RebuildReport {
    RebuildReport {
        queue_id: job.job.id,
        built_at: Utc::now().naive_utc(),
        build_log: DUMMY_BUILD_LOG.to_string().into_bytes(),
        status: BuildStatus::Fail,
        artifacts: vec![],
    }
}

pub fn good_rebuild_report(job: &QueuedJobWithArtifacts) -> RebuildReport {
    let mut artifacts = Vec::new();
    for artifact in job.artifacts.clone() {
        artifacts.push(RebuildArtifactReport {
            name: artifact.name.clone(),
            diffoscope: None,
            status: ArtifactStatus::Good,
            attestation: None,
        });
    }

    RebuildReport {
        queue_id: job.job.id,
        built_at: Utc::now().naive_utc(),
        build_log: DUMMY_BUILD_LOG.to_string().into_bytes(),
        status: BuildStatus::Good,
        artifacts,
    }
}

pub async fn good_rebuild_report_with_signed_attestation(
    job: &QueuedJobWithArtifacts,
) -> RebuildReport {
    let input = job.job.url.rsplit_once("/").unwrap().1;

    let mut artifacts = Vec::new();
    for artifact in job.artifacts.clone() {
        let attestation = create_dummy_signed_attestation(input, &artifact.name);

        artifacts.push(RebuildArtifactReport {
            name: artifact.name.clone(),
            diffoscope: None,
            status: ArtifactStatus::Good,
            attestation: Some(zstd_compress(attestation.as_bytes()).await.unwrap()),
        });
    }

    RebuildReport {
        queue_id: job.job.id,
        built_at: Utc::now().naive_utc(),
        build_log: DUMMY_BUILD_LOG.to_string().into_bytes(),
        status: BuildStatus::Good,
        artifacts,
    }
}

pub async fn good_rebuild_report_with_unsigned_attestation(
    job: &QueuedJobWithArtifacts,
) -> RebuildReport {
    let input = job.job.url.rsplit_once("/").unwrap().1;

    let mut artifacts = Vec::new();
    for artifact in job.artifacts.clone() {
        let attestation = create_dummy_unsigned_attestation(input, &artifact.name);

        artifacts.push(RebuildArtifactReport {
            name: artifact.name.clone(),
            diffoscope: None,
            status: ArtifactStatus::Good,
            attestation: Some(zstd_compress(attestation.as_bytes()).await.unwrap()),
        });
    }

    RebuildReport {
        queue_id: job.job.id,
        built_at: Utc::now().naive_utc(),
        build_log: DUMMY_BUILD_LOG.to_string().into_bytes(),
        status: BuildStatus::Good,
        artifacts,
    }
}
