use crate::schema::stats;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, Selectable, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(table_name = stats)]
pub struct StatsSnapshot {
    pub id: i32,
    pub captured_at: NaiveDateTime,
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub architecture: Option<String>,
    pub good: i32,
    pub bad: i32,
    pub fail: i32,
    pub unknown: i32,
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(table_name = stats)]
pub struct NewStatsSnapshot {
    pub captured_at: NaiveDateTime,
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub architecture: Option<String>,
    pub good: i32,
    pub bad: i32,
    pub fail: i32,
    pub unknown: i32,
}

impl NewStatsSnapshot {
    pub fn insert(&self, connection: &mut SqliteConnection) -> Result<i32> {
        let id = diesel::insert_into(stats::table)
            .values(self)
            .returning(stats::id)
            .get_results::<i32>(connection)?;

        Ok(id[0])
    }
}
