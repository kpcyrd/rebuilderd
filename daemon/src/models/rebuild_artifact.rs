use crate::models::Rebuild;
use crate::schema::*;
use diesel::prelude::*;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, Associations, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(belongs_to(Rebuild))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = rebuild_artifacts)]
pub struct RebuildArtifact {
    pub id: i32,
    pub rebuild_id: i32,
    pub name: String,
    pub diffoscope: Option<Vec<u8>>,
    pub attestation: Option<Vec<u8>>,
    pub status: Option<String>,
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = rebuild_artifacts)]
pub struct NewRebuildArtifact {
    pub rebuild_id: i32,
    pub name: String,
    pub diffoscope: Option<Vec<u8>>,
    pub attestation: Option<Vec<u8>>,
    pub status: Option<String>,
}

impl NewRebuildArtifact {
    pub fn insert(&self, connection: &mut SqliteConnection) -> Result<()> {
        diesel::insert_into(rebuild_artifacts::table)
            .values(self)
            .execute(connection)?;

        Ok(())
    }
}
