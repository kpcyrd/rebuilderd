use crate::models::BuildInput;
use crate::schema::*;
use chrono::NaiveDateTime;
use diesel::prelude::*;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, Associations, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(belongs_to(BuildInput))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = rebuilds)]
pub struct Rebuild {
    pub id: i32,
    pub build_input_id: i32,
    pub started_at: Option<NaiveDateTime>,
    pub built_at: Option<NaiveDateTime>,
    pub build_log: Vec<u8>,
    pub status: Option<String>,
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(table_name = rebuilds)]
pub struct NewRebuild {
    pub build_input_id: i32,
    pub started_at: Option<NaiveDateTime>,
    pub built_at: Option<NaiveDateTime>,
    pub build_log: Vec<u8>,
    pub status: Option<String>,
}

impl NewRebuild {
    pub fn insert(&self, connection: &mut SqliteConnection) -> Result<i32> {
        let id = diesel::insert_into(rebuilds::table)
            .values(self)
            .returning(rebuilds::id)
            .get_results::<i32>(connection)?;

        Ok(id[0])
    }
}
