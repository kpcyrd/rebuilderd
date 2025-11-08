#[cfg(feature = "diesel")]
use diesel::Queryable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTagRequest {
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTagRuleRequest {
    pub name_pattern: String,
    pub version_pattern: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(Queryable))]
#[cfg_attr(feature = "diesel", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
pub struct TagRule {
    pub id: i32,
    pub name_pattern: String,
    pub version_pattern: Option<String>,
}
