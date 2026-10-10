use crate::models::StatsSnapshot;
use crate::schema::stats_categories;
use diesel::prelude::*;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, Selectable, Associations, Clone, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(belongs_to(StatsSnapshot, foreign_key = stats_id))]
#[diesel(table_name = stats_categories)]
pub struct StatsCategory {
    pub id: i32,
    pub stats_id: i32,
    pub category: String,
    pub count: i32,
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(table_name = stats_categories)]
pub struct NewStatsCategory {
    pub stats_id: i32,
    pub category: String,
    pub count: i32,
}

impl NewStatsCategory {
    pub fn insert_batch(rows: &[Self], connection: &mut SqliteConnection) -> Result<()> {
        diesel::insert_into(stats_categories::table)
            .values(rows)
            .execute(connection)?;
        Ok(())
    }
}
