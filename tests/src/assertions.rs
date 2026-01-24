use rebuilderd_common::api::v1::{
    BinaryPackage, PackageReport, QueuedJob, SourcePackage, SourcePackageReport,
};

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

pub fn assert_source_package_is_in_report(
    source_package: &SourcePackage,
    package_report: &PackageReport,
) {
    assert_eq!(source_package.distribution, package_report.distribution);
    assert_eq!(source_package.release, package_report.release);
    assert_eq!(source_package.component, package_report.component);

    let found_package = package_report
        .packages
        .iter()
        .find(|pr| pr.name == source_package.name && pr.version == source_package.version);

    assert!(found_package.is_some())
}

pub fn assert_binary_package_is_in_report(
    binary_package: &BinaryPackage,
    package_report: &PackageReport,
) {
    assert_eq!(binary_package.distribution, package_report.distribution);
    assert_eq!(binary_package.release, package_report.release);
    assert_eq!(binary_package.component, package_report.component);

    let found_package = package_report
        .packages
        .iter()
        .flat_map(|p| &p.artifacts)
        .find(|pa| {
            pa.name == binary_package.name
                && pa.version == binary_package.version
                && pa.architecture == binary_package.architecture
        });

    assert!(found_package.is_some())
}
