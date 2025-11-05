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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityFilter {
    pub name: Option<String>,
    pub name_starts_with: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessFilter {
    pub seen_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusFilter {
    #[serde(default, deserialize_with = "deserialize_comma_separated")]
    pub status: Option<Vec<String>>,
}

fn deserialize_comma_separated<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    Ok(s.map(|s| {
        s.split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    }))
}
