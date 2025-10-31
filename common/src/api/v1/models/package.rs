use crate::api::v1::{ArtifactStatus, BuildStatus};
use chrono::NaiveDateTime;
#[cfg(feature = "diesel")]
use diesel::Queryable;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageReport {
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architecture: String,
    pub packages: Vec<SourcePackageReport>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePackageReport {
    pub name: String,
    pub version: String,
    pub url: String,
    pub artifacts: Vec<BinaryPackageReport>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryPackageReport {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(Queryable))]
#[cfg_attr(feature = "diesel", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
pub struct SourcePackage {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub status: Option<BuildStatus>,
    pub build_id: Option<i32>,
    pub last_seen: NaiveDateTime,
    pub seen_in_last_sync: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(Queryable))]
#[cfg_attr(feature = "diesel", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
pub struct BinaryPackage {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architecture: String,
    pub url: String,
    pub status: Option<ArtifactStatus>,
    pub build_id: Option<i32>,
    pub artifact_id: Option<i32>,
    pub diffoscope_log_id: Option<i32>,
    pub attestation_log_id: Option<i32>,
    pub last_seen: NaiveDateTime,
    pub seen_in_last_sync: bool,
}
