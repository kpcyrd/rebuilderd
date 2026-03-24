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

    #[serde(with = "string_separated_vec")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub release: Vec<String>,
    pub component: Option<String>,
    pub architecture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityFilter {
    pub name: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessFilter {
    pub seen_only: Option<bool>,
}

mod string_separated_vec {
    use serde::de::Error;
    use serde::{Deserializer, Serializer, de};
    use std::fmt::Formatter;

    const SEPARATOR: &str = ",";

    pub fn serialize<S>(value: &Vec<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let serialized = value.join(SEPARATOR);
        serializer.serialize_str(&serialized)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringSeparatedVisitor;
        impl<'de> de::Visitor<'de> for StringSeparatedVisitor {
            type Value = Vec<String>;

            fn expecting(
                &self,
                formatter: &mut Formatter<'_>,
            ) -> std::result::Result<(), std::fmt::Error> {
                write!(formatter, "a string")
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: Error,
            {
                self.visit_str(&v)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(v.split(SEPARATOR).map(|s| s.to_string()).collect())
            }
        }

        deserializer.deserialize_str(StringSeparatedVisitor)
    }
}
