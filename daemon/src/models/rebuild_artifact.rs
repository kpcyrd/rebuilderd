use crate::models::Rebuild;
use crate::schema::*;
use diesel::prelude::*;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = diffoscope_logs)]
pub struct DiffoscopeLog {
    pub id: i32,
    pub diffoscope_log: Vec<u8>,
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(table_name = diffoscope_logs)]
pub struct NewDiffoscopeLog {
    pub diffoscope_log: Vec<u8>,
}

impl NewDiffoscopeLog {
    pub fn insert(&self, connection: &mut SqliteConnection) -> Result<i32> {
        let id = diesel::insert_into(diffoscope_logs::table)
            .values(self)
            .returning(diffoscope_logs::id)
            .get_results::<i32>(connection)?;

        Ok(id[0])
    }
}

#[derive(Identifiable, Queryable, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = attestation_logs)]
pub struct AttestationLog {
    pub id: i32,
    pub attestation_log: Vec<u8>,
}

impl AttestationLog {
    pub fn get_for_artifact(
        build_id: i32,
        artifact_id: i32,
        connection: &mut SqliteConnection,
    ) -> Result<Option<Vec<u8>>> {
        let attestation = rebuilds::table
            .inner_join(rebuild_artifacts::table.left_join(attestation_logs::table))
            .filter(rebuilds::id.is(build_id))
            .filter(rebuild_artifacts::id.is(artifact_id))
            .select(attestation_logs::attestation_log.nullable())
            .first::<Option<Vec<u8>>>(connection)
            .optional()?;

        Ok(attestation.flatten())
    }
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(table_name = attestation_logs)]
pub struct NewAttestationLog {
    pub attestation_log: Vec<u8>,
}

impl NewAttestationLog {
    pub fn insert(&self, connection: &mut SqliteConnection) -> Result<i32> {
        let id = diesel::insert_into(attestation_logs::table)
            .values(self)
            .returning(attestation_logs::id)
            .get_results::<i32>(connection)?;

        Ok(id[0])
    }
}

#[derive(Identifiable, Queryable, Associations, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(belongs_to(Rebuild))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = rebuild_artifacts)]
pub struct RebuildArtifact {
    pub id: i32,
    pub rebuild_id: i32,
    pub name: String,
    pub diffoscope_log_id: Option<i32>,
    pub attestation_log_id: Option<i32>,
    pub status: Option<String>,
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = rebuild_artifacts)]
pub struct NewRebuildArtifact {
    pub rebuild_id: i32,
    pub name: String,
    pub diffoscope_log_id: Option<i32>,
    pub attestation_log_id: Option<i32>,
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
