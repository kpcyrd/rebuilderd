use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OriginFilter {
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IdentityFilter {
    pub name: Option<String>,
    pub version: Option<String>,
}
