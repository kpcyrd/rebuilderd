use crate::actions::*;
use crate::assertions::*;
use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use chrono::Utc;
use rebuilderd_common::api::v1::{
    BuildRestApi, BuildStatus, IdentityFilter, OriginFilter, PackageReport, PackageRestApi,
    Priority, QueueRestApi,
};
use rebuilderd_common::config::ConfigFile;
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn fails_if_no_admin_authentication_is_provided(isolated_server: IsolatedServer) {
    let mut client = isolated_server.client;

    // zero out key
    client.auth_cookie("");
    let result = client.submit_package_report(&single_package_report()).await;

    assert!(result.is_err());
}

#[rstest]
#[tokio::test]
pub async fn can_submit_package_report_with_single_package(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();
}

#[rstest]
#[tokio::test]
pub async fn has_correct_packages_after_import_of_single_packages(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let report = single_package_report();

    client.submit_package_report(&report).await.unwrap();

    let mut source_packages = client
        .get_source_packages(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, source_packages.len());

    let source_package = source_packages.pop().unwrap();
    assert_source_package_is_in_report(&source_package, &report);

    let mut binary_packages = client
        .get_binary_packages(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, binary_packages.len());

    let binary_package = binary_packages.pop().unwrap();
    assert_binary_package_is_in_report(&binary_package, &report);
}
#[rstest]
#[tokio::test]
pub async fn can_submit_package_report_with_single_package_with_multiple_artifacts(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    client
        .submit_package_report(&single_package_with_multiple_artifacts_report())
        .await
        .unwrap();
}

#[rstest]
#[tokio::test]
pub async fn has_correct_artifacts_after_import_of_single_package_with_multiple_artifacts(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    let report = single_package_with_multiple_artifacts_report();

    client.submit_package_report(&report).await.unwrap();

    let mut source_packages = client
        .get_source_packages(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, source_packages.len());

    let source_package = source_packages.pop().unwrap();
    assert_source_package_is_in_report(&source_package, &report);

    let binary_packages = client
        .get_binary_packages(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(2, binary_packages.len());

    for package in binary_packages {
        assert_binary_package_is_in_report(&package, &report);
    }
}

#[rstest]
#[tokio::test]
pub async fn can_submit_package_report_with_multiple_packages(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    client
        .submit_package_report(&multiple_package_report())
        .await
        .unwrap();
}

#[rstest]
#[tokio::test]
pub async fn has_correct_packages_after_import_of_multiple_packages(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    let report = multiple_package_report();

    client.submit_package_report(&report).await.unwrap();

    let mut source_packages = client
        .get_source_packages(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(2, source_packages.len());

    let source_package = source_packages.pop().unwrap();
    assert_source_package_is_in_report(&source_package, &report);

    let binary_packages = client
        .get_binary_packages(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(3, binary_packages.len());

    for package in binary_packages {
        assert_binary_package_is_in_report(&package, &report);
    }
}

#[rstest]
#[tokio::test]
pub async fn job_is_queued_after_import_of_new_package(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(jobs.len(), 1)
}

#[rstest]
#[tokio::test]
pub async fn queued_job_from_new_package_has_correct_data(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let package_report = single_package_report();
    client.submit_package_report(&package_report).await.unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    let package = &package_report.packages[0];

    assert_job_matches_package(&package_report, package, &job);
    assert_eq!(None, job.next_retry);
    assert_eq!(Priority::default(), job.priority);
    assert_eq!(None, job.started_at);
    assert!(job.queued_at <= Utc::now().naive_utc());
}

#[rstest]
#[tokio::test]
pub async fn job_is_queued_after_import_of_new_package_with_multiple_artifacts(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    setup_single_imported_package_with_multiple_artifacts(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(jobs.len(), 1)
}

#[rstest]
#[tokio::test]
pub async fn queued_job_from_new_package_with_multiple_artifacts_has_correct_data(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    let package_report = single_package_with_multiple_artifacts_report();
    client.submit_package_report(&package_report).await.unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    let package = &package_report.packages[0];

    assert_job_matches_package(&package_report, package, &job);
    assert_eq!(None, job.next_retry);
    assert_eq!(Priority::default(), job.priority);
    assert_eq!(None, job.started_at);
    assert!(job.queued_at <= Utc::now().naive_utc());
}

#[rstest]
#[tokio::test]
pub async fn additional_job_is_not_queued_after_reimport(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;
    import_single_package(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(jobs.len(), 1)
}

#[rstest]
#[tokio::test]
pub async fn queued_job_from_reimported_package_has_correct_data(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let package_report = single_package_report();
    client.submit_package_report(&package_report).await.unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    let package = &package_report.packages[0];

    assert_job_matches_package(&package_report, package, &job);
    assert_eq!(None, job.next_retry);
    assert_eq!(Priority::default(), job.priority);
    assert_eq!(None, job.started_at);
    assert!(job.queued_at <= Utc::now().naive_utc());
}

#[rstest]
#[tokio::test]
pub async fn initial_delay_sets_next_retry_correctly_for_new_packages(
    #[with(None, None, Some(60))] config_file: ConfigFile,
    #[with(config_file.clone())] isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert!(job.next_retry.is_some());

    let difference = job.next_retry.unwrap() - job.queued_at;
    assert_eq!(config_file.schedule.initial_delay(), difference);
}

#[rstest]
#[tokio::test]
pub async fn initial_delay_does_not_affect_existing_packages(
    #[with(None, None, Some(60))] config_file: ConfigFile,
    #[with(config_file.clone())] isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert!(job.next_retry.is_some());

    let difference = job.next_retry.unwrap() - job.queued_at;
    assert_eq!(config_file.schedule.initial_delay(), difference);

    // rerun, make sure the delay did not change
    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();

    let old_queued_at = job.queued_at;
    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert!(job.next_retry.is_some());

    let difference = job.next_retry.unwrap() - old_queued_at;
    assert_eq!(config_file.schedule.initial_delay(), difference);
}

#[rstest]
#[tokio::test]
pub async fn importing_the_same_package_multiple_times_is_idempotent(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    let report = single_package_report();

    client.submit_package_report(&report).await.unwrap();

    // and again
    client.submit_package_report(&report).await.unwrap();

    let mut source_packages = client
        .get_source_packages(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, source_packages.len());

    let source_package = source_packages.pop().unwrap();
    assert_source_package_is_in_report(&source_package, &report);

    let mut binary_packages = client
        .get_binary_packages(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, binary_packages.len());

    let binary_package = binary_packages.pop().unwrap();
    assert_binary_package_is_in_report(&binary_package, &report);
}

#[rstest]
#[tokio::test]
pub async fn friend_source_packages_are_imported_independently(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();

    client
        .submit_package_report(&single_package_report_from_different_release())
        .await
        .unwrap();

    let source_packages = client
        .get_source_packages(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(2, source_packages.len());
}

#[rstest]
#[tokio::test]
pub async fn friend_binary_packages_are_imported_independently(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();

    client
        .submit_package_report(&single_package_report_from_different_release())
        .await
        .unwrap();

    let binary_packages = client
        .get_binary_packages(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(2, binary_packages.len());
}

#[rstest]
#[case(OriginFilter {
        distribution: None,
        release: Some(DUMMY_OTHER_RELEASE.to_string()),
        component: None,
        architecture: None,
    }, single_package_report_from_different_release())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: Some(DUMMY_OTHER_COMPONENT.to_string()),
        architecture: None,
    }, single_package_report_from_different_component())]
#[tokio::test]
pub async fn existing_rebuilds_are_copied_from_friends(
    isolated_server: IsolatedServer,
    #[case] origin_filter: OriginFilter,
    #[case] extra_packages: PackageReport,
) {
    let client = isolated_server.client;

    // first, a single package with a good rebuild
    setup_single_good_rebuild(&client).await;

    // then, the friend of that package
    let friend_identity = IdentityFilter {
        name: Some(DUMMY_BINARY_PACKAGE.to_string()),
        version: Some(DUMMY_BINARY_PACKAGE_VERSION.to_string()),
    };

    client.submit_package_report(&extra_packages).await.unwrap();

    let mut rebuilds = client
        .get_builds(None, Some(&origin_filter), Some(&friend_identity))
        .await
        .unwrap()
        .records;

    assert_eq!(1, rebuilds.len());

    let rebuild = rebuilds.pop().unwrap();
    assert_eq!(BuildStatus::Good, rebuild.status.unwrap());
}

#[rstest]
#[case(OriginFilter{
        distribution: Some(DUMMY_OTHER_DISTRIBUTION.to_string()),
        release: None,
        component: None,
        architecture: None,
    },
    single_package_report_from_different_distribution())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: None,
        architecture: Some(DUMMY_OTHER_ARCHITECTURE.to_string()),
    }, single_package_report_from_different_architecture())]
#[tokio::test]
pub async fn existing_rebuilds_are_not_copied_from_nonfriends(
    isolated_server: IsolatedServer,
    #[case] origin_filter: OriginFilter,
    #[case] extra_packages: PackageReport,
) {
    let client = isolated_server.client;

    // first, a single package with a good rebuild
    setup_single_good_rebuild(&client).await;

    // then, the nonfriends of that package
    let friend_identity = IdentityFilter {
        name: Some(DUMMY_BINARY_PACKAGE.to_string()),
        version: Some(DUMMY_BINARY_PACKAGE_VERSION.to_string()),
    };

    client.submit_package_report(&extra_packages).await.unwrap();

    let rebuilds = client
        .get_builds(None, Some(&origin_filter), Some(&friend_identity))
        .await
        .unwrap()
        .records;

    assert!(rebuilds.is_empty());
}

#[rstest]
#[tokio::test]
pub async fn next_retry_is_not_set_for_new_packages(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert!(job.next_retry.is_none());
}

#[rstest]
#[tokio::test]
pub async fn next_retry_is_unchanged_for_reimports(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    // one bad rebuild, which gets next_retry set
    setup_single_bad_rebuild(&client).await;

    let old_job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    client
        .submit_package_report(&single_package_report())
        .await
        .unwrap();

    let job = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records
        .pop()
        .unwrap();

    assert_eq!(old_job.next_retry, job.next_retry);
}

#[rstest]
#[case(OriginFilter {
        distribution: None,
        release: Some(DUMMY_OTHER_RELEASE.to_string()),
        component: None,
        architecture: None,
    }, single_package_report_from_different_release())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: Some(DUMMY_OTHER_COMPONENT.to_string()),
        architecture: None,
    }, single_package_report_from_different_component())]
#[tokio::test]
pub async fn retry_count_is_independent_of_friends(
    isolated_server: IsolatedServer,
    #[case] origin_filter: OriginFilter,
    #[case] extra_packages: PackageReport,
) {
    let client = isolated_server.client;

    // one bad rebuild, which would have a retry count of 1
    setup_single_bad_rebuild(&client).await;

    // a friend, which will get the builds copied
    client.submit_package_report(&extra_packages).await.unwrap();

    let mut builds = client
        .get_builds(None, Some(&origin_filter), None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, builds.len());

    let build = builds.pop().unwrap();
    assert_eq!(0, build.retries);
}

#[rstest]
#[case(OriginFilter {
        distribution: None,
        release: Some(DUMMY_OTHER_RELEASE.to_string()),
        component: None,
        architecture: None,
    }, single_package_report_from_different_release())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: Some(DUMMY_OTHER_COMPONENT.to_string()),
        architecture: None,
    }, single_package_report_from_different_component())]
#[tokio::test]
pub async fn package_is_not_queued_if_any_friend_is_marked_good(
    isolated_server: IsolatedServer,
    #[case] origin_filter: OriginFilter,
    #[case] extra_packages: PackageReport,
) {
    let client = isolated_server.client;

    setup_single_good_rebuild(&client).await;

    client.submit_package_report(&extra_packages).await.unwrap();

    let jobs = client
        .get_queued_jobs(None, Some(&origin_filter), None)
        .await
        .unwrap()
        .records;
    assert!(jobs.is_empty());
}

#[rstest]
#[case(OriginFilter {
        distribution: None,
        release: Some(DUMMY_OTHER_RELEASE.to_string()),
        component: None,
        architecture: None,
    }, single_package_report_from_different_release())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: Some(DUMMY_OTHER_COMPONENT.to_string()),
        architecture: None,
    }, single_package_report_from_different_component())]
#[tokio::test]
pub async fn package_is_not_queued_if_any_friend_is_already_queued(
    isolated_server: IsolatedServer,
    #[case] origin_filter: OriginFilter,
    #[case] extra_packages: PackageReport,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    client.submit_package_report(&extra_packages).await.unwrap();

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;
    assert_eq!(1, jobs.len());

    let jobs = client
        .get_queued_jobs(None, Some(&origin_filter), None)
        .await
        .unwrap()
        .records;

    assert!(jobs.is_empty());
}

#[rstest]
#[case(OriginFilter {
        distribution: None,
        release: Some(DUMMY_OTHER_RELEASE.to_string()),
        component: None,
        architecture: None,
    }, single_package_report_from_different_release())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: Some(DUMMY_OTHER_COMPONENT.to_string()),
        architecture: None,
    }, single_package_report_from_different_component())]
#[tokio::test]
pub async fn package_is_not_queued_if_any_friend_is_already_past_max_retries(
    #[with(None, Some(1), None)] config_file: ConfigFile,
    #[with(config_file.clone())] isolated_server: IsolatedServer,
    #[case] origin_filter: OriginFilter,
    #[case] extra_packages: PackageReport,
) {
    let client = isolated_server.client;

    setup_build_ready_database(&client).await;

    // first attempt, get and report
    report_bad_rebuild(&client).await;

    // ensure package is not enqueued after failed attempt
    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert!(jobs.is_empty());

    // import a friend
    client.submit_package_report(&extra_packages).await.unwrap();

    // ensure friend did not get enqueued
    let jobs = client
        .get_queued_jobs(None, Some(&origin_filter), None)
        .await
        .unwrap()
        .records;

    assert!(jobs.is_empty());
}

#[rstest]
#[tokio::test]
pub async fn drops_available_jobs_not_in_current_sync(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    import_single_package(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, jobs.len());

    import_single_package_with_multiple_artifacts(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, jobs.len());
}

#[rstest]
#[tokio::test]
pub async fn enqueues_jobs_for_all_packages_in_sync(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    import_multiple_packages(&client).await;

    let jobs = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(2, jobs.len());
}
