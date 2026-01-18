use crate::data::*;
use crate::fixtures::server::IsolatedServer;
use crate::fixtures::*;
use crate::setup::*;
use rebuilderd_common::api::v1::{
    IdentityFilter, OriginFilter, PackageReport, PackageRestApi, Page,
};
use rstest::rstest;

#[rstest]
#[tokio::test]
pub async fn returns_no_results_for_empty_database(isolated_server: IsolatedServer) {
    let results = isolated_server
        .client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert!(results.is_empty())
}

#[rstest]
#[tokio::test]
pub async fn returns_single_result_for_database_with_single_artifact(
    isolated_server: IsolatedServer,
) {
    setup_single_imported_package(&isolated_server.client).await;

    let results = isolated_server
        .client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(1, results.len())
}

#[rstest]
#[tokio::test]
pub async fn returns_multiple_results_for_database_with_multiple_artifacts(
    isolated_server: IsolatedServer,
) {
    setup_single_imported_package_with_multiple_artifacts(&isolated_server.client).await;

    let results = isolated_server
        .client
        .get_binary_packages(None, None, None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(2, results.len())
}

#[rstest]
#[tokio::test]
pub async fn can_paginate(isolated_server: IsolatedServer) {
    let client = isolated_server.client;

    setup_single_imported_package_with_multiple_artifacts(&client).await;

    let mut page = Page {
        limit: Some(1),
        before: None,
        after: None,
        sort: None,
        direction: None,
    };

    let mut first_page = client
        .get_binary_packages(Some(&page), None, None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(1, first_page.len());

    let result = first_page.pop().unwrap();
    page.after = Some(result.id);

    let mut next_page = client
        .get_binary_packages(Some(&page), None, None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(1, next_page.len());

    let result = next_page.pop().unwrap();
    page.after = Some(result.id);

    let next_page = client
        .get_binary_packages(Some(&page), None, None)
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
    single_package_report_from_different_distribution())]
#[case(OriginFilter {
        distribution: None,
        release: Some(DUMMY_RELEASE.to_string()),
        component: None,
        architecture: None,
    }, single_package_report_from_different_release())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: Some(DUMMY_COMPONENT.to_string()),
        architecture: None,
    }, single_package_report_from_different_component())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: None,
        architecture: Some(DUMMY_ARCHITECTURE.to_string()),
    }, single_package_report_from_different_architecture())]
#[case(OriginFilter {
        distribution: None,
        release: None,
        component: None,
        architecture: None,
    }, single_package_with_multiple_artifacts_report())]
#[tokio::test]
pub async fn returns_result_for_matching_origin_filter(
    isolated_server: IsolatedServer,
    #[case] origin_filter: OriginFilter,
    #[case] extra_packages: PackageReport,
) {
    setup_single_imported_package_with_multiple_artifacts(&isolated_server.client).await;

    isolated_server
        .client
        .submit_package_report(&extra_packages)
        .await
        .unwrap();

    let results = isolated_server
        .client
        .get_binary_packages(None, Some(&origin_filter), None)
        .await
        .map(|p| p.records)
        .unwrap();

    assert_eq!(2, results.len());

    let artifact_1 = &results[0];
    let artifact_2 = &results[1];

    if let Some(distribution) = origin_filter.distribution {
        assert_eq!(distribution, artifact_1.distribution);
        assert_eq!(distribution, artifact_2.distribution);
    }

    if let Some(release) = origin_filter.release {
        assert_eq!(Some(&release), artifact_1.release.as_ref());
        assert_eq!(Some(&release), artifact_2.release.as_ref());
    }

    if let Some(component) = origin_filter.component {
        assert_eq!(Some(&component), artifact_1.component.as_ref());
        assert_eq!(Some(&component), artifact_2.component.as_ref());
    }

    if let Some(architecture) = origin_filter.architecture {
        assert_eq!(architecture, artifact_1.architecture);
        assert_eq!(architecture, artifact_2.architecture);
    }
}

#[rstest]
#[case(IdentityFilter{
        name: Some(DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1.to_string()),
        version: None,
    },
    1)]
#[case(IdentityFilter{
        name: Some(DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2.to_string()),
        version: None,
    },
    1)]
#[case(IdentityFilter{
        name: None,
        version: Some(DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1_VERSION.to_string()),
    },
    1)]
#[case(IdentityFilter{
        name: None,
        version: Some(DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2_VERSION.to_string()),
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
    setup_single_imported_package_with_multiple_artifacts(&isolated_server.client).await;

    let results = isolated_server
        .client
        .get_binary_packages(None, None, Some(&identity_filter))
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
