use crate::data::{DUMMY_ARCHITECTURE, DUMMY_COMPONENT, DUMMY_DISTRIBUTION, DUMMY_RELEASE};
use rebuilderd_common::api::v1::{BinaryPackageReport, PackageReport, SourcePackageReport};

pub fn single_package_report() -> PackageReport {
    PackageReport {
        distribution: DUMMY_DISTRIBUTION.to_string(),
        release: Some(DUMMY_RELEASE.to_string()),
        component: Some(DUMMY_COMPONENT.to_string()),
        architecture: DUMMY_ARCHITECTURE.to_string(),
        packages: vec![SourcePackageReport {
            name: "baz".to_string(),
            version: "1".to_string(),
            url: "https://placeholder.org/foo-1.buildinfo.txt".to_string(),
            artifacts: vec![BinaryPackageReport {
                name: "baz".to_string(),
                version: "1".to_string(),
                architecture: DUMMY_ARCHITECTURE.to_string(),
                url: "https://placeholder.org/foo-1.tar.zst".to_string(),
            }],
        }],
    }
}

pub fn single_package_with_multiple_artifacts_report() -> PackageReport {
    PackageReport {
        distribution: DUMMY_DISTRIBUTION.to_string(),
        release: Some(DUMMY_RELEASE.to_string()),
        component: Some(DUMMY_COMPONENT.to_string()),
        architecture: DUMMY_ARCHITECTURE.to_string(),
        packages: vec![SourcePackageReport {
            name: "foobar".to_string(),
            version: "2".to_string(),
            url: "https://placeholder.org/foobar-1.2.3-4.buildinfo.txt".to_string(),
            artifacts: vec![
                BinaryPackageReport {
                    name: "foo".to_string(),
                    version: "0.1.2".to_string(),
                    architecture: DUMMY_ARCHITECTURE.to_string(),
                    url: "https://placeholder.org/foo-0.1.2.tar.zst".to_string(),
                },
                BinaryPackageReport {
                    name: "bar".to_string(),
                    version: "3.4.5".to_string(),
                    architecture: DUMMY_ARCHITECTURE.to_string(),
                    url: "https://placeholder.org/bar-3.4.5.tar.zst".to_string(),
                },
            ],
        }],
    }
}
