use rebuilderd_common::api::v1::{PackageReport, QueuedJob, SourcePackageReport};

pub fn assert_job_matches_package(
    package_report: &PackageReport,
    package: &SourcePackageReport,
    job: &QueuedJob,
) {
    assert_eq!(package.name, job.name);
    assert_eq!(package.version, job.version);
    assert_eq!(package_report.distribution, job.distribution);
    assert_eq!(package_report.release, job.release);
    assert_eq!(package_report.component, job.component);
    assert_eq!(package_report.architecture, job.architecture);
    assert_eq!(package_report.distribution, job.backend);
    assert_eq!(package.url, job.url);
}
