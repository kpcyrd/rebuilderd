use crate::actions::*;
use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use rebuilderd_common::api::v1::{
    IdentityFilter, OriginFilter, PackageReport, PackageRestApi, Page, QueueRestApi,
};
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_empty_database(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    let results = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert!(results.is_empty());
}

#[rstest]
#[tokio::test]
pub async fn returns_single_result_for_database_with_single_job(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    import_single_package(&client).await;

    let results = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(1, results.len());
}

#[rstest]
#[tokio::test]
pub async fn returns_multiple_results_for_database_with_multiple_jobs(
    isolated_server: IsolatedServer,
) {
    let client = isolated_server.client;

    import_multiple_packages(&client).await;

    let results = client
        .get_queued_jobs(None, None, None)
        .await
        .unwrap()
        .records;

    assert_eq!(2, results.len());
}

#[rstest]
#[tokio::test]
pub async fn does_not_need_authentication(isolated_server: IsolatedServer) {
    let mut client = isolated_server.client;

    import_multiple_packages(&client).await;

    // zero out keys
    client.auth_cookie("");
    client.worker_key("");
    client.signup_secret("");

    let result = client.get_queued_jobs(None, None, None).await;

    assert!(result.is_ok());
}

#[rstest]
#[tokio::test]
pub async fn can_paginate(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    import_multiple_packages(&client).await;

    let mut page = Page {
        limit: Some(1),
        before: None,
        after: None,
        sort: None,
        direction: None,
    };

    let mut first_page = client
        .get_queued_jobs(Some(&page), None, None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(1, first_page.len());

    let result = first_page.pop().unwrap();
    page.after = Some(result.id);

    let mut next_page = client
        .get_queued_jobs(Some(&page), None, None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(1, next_page.len());

    let result = next_page.pop().unwrap();
    page.after = Some(result.id);

    let next_page = client
        .get_queued_jobs(Some(&page), None, None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert!(next_page.is_empty());
}

#[rstest]
#[case(OriginFilter{
        distribution: Some(DUMMY_DISTRIBUTION.to_string()),
        release: None,
        component: None,
        architecture: None,
    },
    single_package_report_from_different_distribution(),
    1)]
#[case(OriginFilter {
        distribution: None,
        release: Some(DUMMY_RELEASE.to_string()),
        component: None,
        architecture: None,
    }, single_package_report_from_different_release(),
    1)]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: Some(DUMMY_COMPONENT.to_string()),
        architecture: None,
    }, single_package_report_from_different_component(),
    1)]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: None,
        architecture: Some(DUMMY_ARCHITECTURE.to_string()),
    }, single_package_report_from_different_architecture(),
    1)]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: None,
        architecture: None,
    }, single_package_report_from_different_distribution(),
    2)]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: None,
        architecture: None,
    }, single_package_report(),
    1)]
#[tokio::test]
pub async fn returns_result_for_matching_origin_filter(
    isolated_server: IsolatedServer,
    #[case] origin_filter: OriginFilter,
    #[case] extra_packages: PackageReport,
    #[case] expected_count: usize,
) {
    let client = isolated_server.client;

    setup_single_imported_package(&client).await;

    client.submit_package_report(&extra_packages).await.unwrap();

    let results = client
        .get_queued_jobs(None, Some(&origin_filter), None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(expected_count, results.len());

    if let Some(distribution) = origin_filter.distribution {
        for result in &results {
            assert_eq!(distribution, result.distribution);
        }
    }

    if let Some(release) = origin_filter.release {
        for result in &results {
            assert_eq!(Some(&release), result.release.as_ref());
        }
    }

    if let Some(component) = origin_filter.component {
        for result in &results {
            assert_eq!(Some(&component), result.component.as_ref());
        }
    }
}

#[rstest]
#[case(IdentityFilter{
        name: Some(DUMMY_SOURCE_PACKAGE.to_string()),
        version: None,
    },
    1)]
#[case(IdentityFilter{
        name: Some(DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE.to_string()),
        version: None,
    },
    1)]
#[case(IdentityFilter{
        name: None,
        version: Some(DUMMY_SOURCE_PACKAGE_VERSION.to_string()),
    },
    1)]
#[case(IdentityFilter{
        name: None,
        version: Some(DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE_VERSION.to_string()),
    },
    1)]
#[case(IdentityFilter{
        name: None,
        version: None,
    },
    2)]
#[tokio::test]
pub async fn returns_result_for_matching_identity_filter(
    isolated_server: IsolatedServer,
    #[case] identity_filter: IdentityFilter,
    #[case] expected_count: usize,
) {
    let client = isolated_server.client;

    setup_multiple_imported_packages(&client).await;

    let results = client
        .get_queued_jobs(None, None, Some(&identity_filter))
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(expected_count, results.len());

    if let Some(name) = identity_filter.name {
        for package in &results {
            assert_eq!(name, package.name);
        }
    }

    if let Some(version) = identity_filter.version {
        for package in &results {
            assert_eq!(version, package.version);
        }
    }
}
