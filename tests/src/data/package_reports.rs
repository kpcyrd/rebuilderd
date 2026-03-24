use crate::data::*;
use rebuilderd_common::api::v1::{BinaryPackageReport, PackageReport, SourcePackageReport};

pub const DUMMY_SOURCE_PACKAGE: &str = "foo";
pub const DUMMY_SOURCE_PACKAGE_VERSION: &str = "1";
pub const DUMMY_SOURCE_PACKAGE_URL: &str = "https://placeholder.org/foo-1.buildinfo.txt";
pub const DUMMY_BINARY_PACKAGE: &str = "foo";
pub const DUMMY_BINARY_PACKAGE_VERSION: &str = "1";
pub const DUMMY_BINARY_PACKAGE_URL: &str = "https://placeholder.org/foo-1.tar.zst";

pub fn single_package_report() -> PackageReport {
    PackageReport {
        distribution: DUMMY_DISTRIBUTION.to_string(),
        release: Some(DUMMY_RELEASE.to_string()),
        component: Some(DUMMY_COMPONENT.to_string()),
        architecture: DUMMY_ARCHITECTURE.to_string(),
        packages: vec![SourcePackageReport {
            name: DUMMY_SOURCE_PACKAGE.to_string(),
            version: DUMMY_SOURCE_PACKAGE_VERSION.to_string(),
            url: DUMMY_SOURCE_PACKAGE_URL.to_string(),
            artifacts: vec![BinaryPackageReport {
                name: DUMMY_BINARY_PACKAGE.to_string(),
                version: DUMMY_BINARY_PACKAGE_VERSION.to_string(),
                architecture: DUMMY_ARCHITECTURE.to_string(),
                url: DUMMY_BINARY_PACKAGE_URL.to_string(),
            }],
        }],
    }
}

pub const DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE: &str = "barbaz";
pub const DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE_VERSION: &str = "2";
pub const DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE_URL: &str =
    "https://placeholder.org/barbaz-2.buildinfo.txt";
pub const DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1: &str = "bar";
pub const DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1_VERSION: &str = "3";

pub const DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1_URL: &str = "https://placeholder.org/bar-3.tar.zst";
pub const DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2: &str = "baz";
pub const DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2_VERSION: &str = "4";
pub const DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2_URL: &str = "https://placeholder.org/baz-4.tar.zst";

pub fn single_package_with_multiple_artifacts_report() -> PackageReport {
    PackageReport {
        distribution: DUMMY_DISTRIBUTION.to_string(),
        release: Some(DUMMY_RELEASE.to_string()),
        component: Some(DUMMY_COMPONENT.to_string()),
        architecture: DUMMY_ARCHITECTURE.to_string(),
        packages: vec![SourcePackageReport {
            name: DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE.to_string(),
            version: DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE_VERSION.to_string(),
            url: DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE_URL.to_string(),
            artifacts: vec![
                BinaryPackageReport {
                    name: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1.to_string(),
                    version: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1_VERSION.to_string(),
                    architecture: DUMMY_ARCHITECTURE.to_string(),
                    url: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1_URL.to_string(),
                },
                BinaryPackageReport {
                    name: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2.to_string(),
                    version: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2_VERSION.to_string(),
                    architecture: DUMMY_ARCHITECTURE.to_string(),
                    url: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2_URL.to_string(),
                },
            ],
        }],
    }
}

pub fn single_package_report_from_different_distribution() -> PackageReport {
    PackageReport {
        distribution: DUMMY_OTHER_DISTRIBUTION.to_string(),
        ..single_package_report()
    }
}

pub fn single_package_report_from_different_release() -> PackageReport {
    PackageReport {
        release: Some(DUMMY_OTHER_RELEASE.to_string()),
        ..single_package_report()
    }
}

pub fn single_package_report_from_different_component() -> PackageReport {
    PackageReport {
        component: Some(DUMMY_OTHER_COMPONENT.to_string()),
        ..single_package_report()
    }
}

pub fn single_package_report_from_different_architecture() -> PackageReport {
    PackageReport {
        architecture: DUMMY_OTHER_ARCHITECTURE.to_string(),
        ..single_package_report()
    }
}

pub fn multiple_package_report() -> PackageReport {
    PackageReport {
        distribution: DUMMY_DISTRIBUTION.to_string(),
        release: Some(DUMMY_RELEASE.to_string()),
        component: Some(DUMMY_COMPONENT.to_string()),
        architecture: DUMMY_ARCHITECTURE.to_string(),
        packages: vec![
            SourcePackageReport {
                name: DUMMY_SOURCE_PACKAGE.to_string(),
                version: DUMMY_SOURCE_PACKAGE_VERSION.to_string(),
                url: DUMMY_SOURCE_PACKAGE_URL.to_string(),
                artifacts: vec![BinaryPackageReport {
                    name: DUMMY_BINARY_PACKAGE.to_string(),
                    version: DUMMY_BINARY_PACKAGE_VERSION.to_string(),
                    architecture: DUMMY_ARCHITECTURE.to_string(),
                    url: DUMMY_BINARY_PACKAGE_URL.to_string(),
                }],
            },
            SourcePackageReport {
                name: DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE.to_string(),
                version: DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE_VERSION.to_string(),
                url: DUMMY_MULTI_ARTIFACT_SOURCE_PACKAGE_URL.to_string(),
                artifacts: vec![
                    BinaryPackageReport {
                        name: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1.to_string(),
                        version: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1_VERSION.to_string(),
                        architecture: DUMMY_ARCHITECTURE.to_string(),
                        url: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_1_URL.to_string(),
                    },
                    BinaryPackageReport {
                        name: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2.to_string(),
                        version: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2_VERSION.to_string(),
                        architecture: DUMMY_ARCHITECTURE.to_string(),
                        url: DUMMY_MULTI_ARTIFACT_BINARY_PACKAGE_2_URL.to_string(),
                    },
                ],
            },
        ],
    }
}
