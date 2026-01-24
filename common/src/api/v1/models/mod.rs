mod build;
mod dashboard;
mod meta;
mod package;
mod queue;
mod worker;

pub use build::*;
pub use dashboard::*;
pub use meta::*;
pub use package::*;
pub use queue::*;
use serde::{Deserialize, Serialize};
pub use worker::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub limit: Option<i32>,
    pub before: Option<i32>,
    pub after: Option<i32>,
    pub sort: Option<String>,
    pub direction: Option<SortDirection>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResultPage<T> {
    pub total: i64,
    pub records: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginFilter {
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdentityFilter {
    pub name: Option<String>,
    #[serde(default)]
    pub search_type: SearchType,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchType {
    Exact,
    Contains,
    StartsWith,
}

impl Default for SearchType {
    fn default() -> Self {
        Self::Exact
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessFilter {
    pub seen_only: Option<bool>,
}
