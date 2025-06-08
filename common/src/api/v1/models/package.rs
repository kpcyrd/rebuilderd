use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageReport {
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architecture: String,
    pub packages: Vec<SourcePackageReport>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourcePackageReport {
    pub name: String,
    pub version: String,
    pub url: String,
    pub artifacts: Vec<BinaryPackageReport>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BinaryPackageReport {
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SourcePackage {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architectures: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BinaryPackage {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architecture: String,
    pub url: String,
}
