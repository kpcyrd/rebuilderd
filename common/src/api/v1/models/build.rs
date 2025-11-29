use chrono::NaiveDateTime;
#[cfg(feature = "diesel")]
use diesel::{
    AsExpression, FromSqlRow, Queryable, deserialize::FromSql, serialize::Output, serialize::ToSql,
    sql_types::Text, sqlite::Sqlite, sqlite::SqliteValue,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fmt::Formatter;

#[derive(Debug, Serialize, Deserialize)]
pub struct RebuildReport {
    pub queue_id: i32,
    pub built_at: NaiveDateTime,
    pub build_log: Vec<u8>,
    pub status: BuildStatus,
    pub artifacts: Vec<RebuildArtifactReport>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, clap::ValueEnum)]
#[cfg_attr(feature = "diesel", derive(FromSqlRow, AsExpression))]
#[cfg_attr(feature = "diesel", diesel(sql_type = Text))]
#[cfg_attr(feature = "diesel", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
pub enum BuildStatus {
    #[serde(rename = "GOOD")]
    #[clap(name = "GOOD")]
    Good,

    #[serde(rename = "BAD")]
    #[clap(name = "BAD")]
    Bad,

    #[serde(rename = "FAIL")]
    #[clap(name = "FAIL")]
    Fail,

    #[serde(rename = "UNKWN")]
    #[clap(name = "UNKWN")]
    Unknown,
}

impl BuildStatus {
    pub fn as_str(&self) -> &str {
        match self {
            BuildStatus::Good => "GOOD",
            BuildStatus::Bad => "BAD",
            BuildStatus::Fail => "FAIL",
            BuildStatus::Unknown => "UNKWN",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuildStatusParseError {
    value: String,
}

impl fmt::Display for BuildStatusParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value = &self.value;
        write!(f, "could not parse \"{value}\" as a build status")
    }
}

impl Error for BuildStatusParseError {}

impl TryFrom<&str> for BuildStatus {
    type Error = BuildStatusParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "GOOD" => Ok(BuildStatus::Good),
            "BAD" => Ok(BuildStatus::Bad),
            "FAIL" => Ok(BuildStatus::Fail),
            "UNKWN" => Ok(BuildStatus::Unknown),
            _ => Err(BuildStatusParseError {
                value: value.to_string(),
            }),
        }
    }
}

#[cfg(feature = "diesel")]
impl FromSql<Text, Sqlite> for BuildStatus {
    fn from_sql(bytes: SqliteValue) -> diesel::deserialize::Result<Self> {
        let t = <String as FromSql<Text, Sqlite>>::from_sql(bytes)?;
        Ok(t.as_str().try_into()?)
    }
}

#[cfg(feature = "diesel")]
impl ToSql<Text, Sqlite> for BuildStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> diesel::serialize::Result {
        out.set_value(self.as_str());
        Ok(diesel::serialize::IsNull::No)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RebuildArtifactReport {
    pub name: String,
    pub diffoscope: Option<Vec<u8>>,
    pub attestation: Option<Vec<u8>>,
    pub status: ArtifactStatus,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, clap::ValueEnum)]
#[cfg_attr(feature = "diesel", derive(FromSqlRow, AsExpression))]
#[cfg_attr(feature = "diesel", diesel(sql_type = Text))]
#[cfg_attr(feature = "diesel", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
pub enum ArtifactStatus {
    #[serde(rename = "GOOD")]
    #[clap(name = "GOOD")]
    Good,

    #[serde(rename = "BAD")]
    #[clap(name = "BAD")]
    Bad,

    #[serde(rename = "UNKWN")]
    #[clap(name = "UNKWN")]
    Unknown,
}

impl ArtifactStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ArtifactStatus::Good => "GOOD",
            ArtifactStatus::Bad => "BAD",
            ArtifactStatus::Unknown => "UNKWN",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArtifactStatusParseError {
    value: String,
}

impl fmt::Display for ArtifactStatusParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let value = &self.value;
        write!(f, "could not parse \"{value}\" as an artifact status")
    }
}

impl Error for ArtifactStatusParseError {}

impl TryFrom<&str> for ArtifactStatus {
    type Error = ArtifactStatusParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "GOOD" => Ok(ArtifactStatus::Good),
            "BAD" => Ok(ArtifactStatus::Bad),
            "UNKWN" => Ok(ArtifactStatus::Unknown),
            _ => Err(ArtifactStatusParseError {
                value: value.to_string(),
            }),
        }
    }
}

#[cfg(feature = "diesel")]
impl FromSql<Text, Sqlite> for ArtifactStatus {
    fn from_sql(bytes: SqliteValue) -> diesel::deserialize::Result<Self> {
        let t = <String as FromSql<Text, Sqlite>>::from_sql(bytes)?;
        Ok(t.as_str().try_into()?)
    }
}

#[cfg(feature = "diesel")]
impl ToSql<Text, Sqlite> for ArtifactStatus {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> diesel::serialize::Result {
        out.set_value(self.as_str());
        Ok(diesel::serialize::IsNull::No)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(Queryable))]
#[cfg_attr(feature = "diesel", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
pub struct Rebuild {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architecture: String,
    pub backend: String,
    pub retries: i32,
    pub started_at: Option<NaiveDateTime>,
    pub built_at: Option<NaiveDateTime>,
    pub status: Option<BuildStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "diesel", derive(Queryable))]
#[cfg_attr(feature = "diesel", diesel(check_for_backend(diesel::sqlite::Sqlite)))]
pub struct RebuildArtifact {
    pub id: i32,
    pub name: String,
    pub has_diffoscope: bool,
    pub has_attestation: bool,
    pub status: Option<ArtifactStatus>,
}
