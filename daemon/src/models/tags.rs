use crate::schema::*;
use diesel::{
    AsChangeset, Identifiable, Insertable, OptionalExtension, Queryable, RunQueryDsl, Selectable,
    SelectableHelper, SqliteConnection, SqliteExpressionMethods,
};
use diesel::{ExpressionMethods, QueryDsl};
use rebuilderd_common::errors::*;
use serde::{Deserialize, Serialize};

#[derive(Identifiable, Queryable, AsChangeset, Selectable, Serialize, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = tags)]
pub struct Tag {
    pub id: i32,
    pub tag: String,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = tags)]
pub struct NewTag {
    pub tag: String,
}

impl NewTag {
    pub fn ensure_exists(self, connection: &mut SqliteConnection) -> Result<Tag> {
        use crate::schema::tags::*;

        let inserted = diesel::insert_into(table)
            .values(&self)
            .on_conflict_do_nothing()
            .returning(Tag::as_select())
            .get_result(connection)
            .optional()?;

        if let Some(inserted) = inserted {
            return Ok(inserted);
        }

        let existing = table
            .filter(tag.eq(self.tag))
            .select(Tag::as_select())
            .get_result(connection)?;

        Ok(existing)
    }
}

#[derive(Identifiable, Queryable, Selectable, Serialize, PartialEq, Eq, Debug)]
#[diesel(primary_key(worker_id, tag_id))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = worker_tags)]
pub struct WorkerTag {
    pub worker_id: i32,
    pub tag_id: i32,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = worker_tags)]
pub struct NewWorkerTag {
    pub worker_id: i32,
    pub tag_id: i32,
}

impl NewWorkerTag {
    pub fn ensure_exists(self, connection: &mut SqliteConnection) -> Result<WorkerTag> {
        use crate::schema::worker_tags::*;

        let inserted = diesel::insert_into(table)
            .values(&self)
            .on_conflict_do_nothing()
            .returning(WorkerTag::as_select())
            .get_result(connection)
            .optional()?;

        if let Some(inserted) = inserted {
            return Ok(inserted);
        }

        let existing = table
            .filter(worker_id.eq(self.worker_id))
            .filter(tag_id.eq(self.tag_id))
            .select(WorkerTag::as_select())
            .get_result(connection)?;

        Ok(existing)
    }
}

#[derive(Identifiable, Queryable, AsChangeset, Selectable, Serialize, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = source_package_tag_rules)]
pub struct SourcePackageTagRule {
    pub id: i32,
    pub tag_id: i32,
    pub source_package_name_pattern: String,
    pub source_package_version_pattern: Option<String>,
}

#[derive(Insertable, Serialize, Deserialize, Debug)]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = source_package_tag_rules)]
pub struct NewSourcePackageTagRule {
    pub tag_id: i32,
    pub source_package_name_pattern: String,
    pub source_package_version_pattern: Option<String>,
}

impl NewSourcePackageTagRule {
    pub fn ensure_exists(self, connection: &mut SqliteConnection) -> Result<SourcePackageTagRule> {
        use crate::schema::source_package_tag_rules::*;

        let inserted = diesel::insert_into(table)
            .values(&self)
            .on_conflict_do_nothing()
            .returning(SourcePackageTagRule::as_select())
            .get_result(connection)
            .optional()?;

        if let Some(inserted) = inserted {
            return Ok(inserted);
        }

        let existing = table
            .filter(tag_id.eq(self.tag_id))
            .filter(source_package_name_pattern.eq(self.source_package_name_pattern))
            .filter(source_package_version_pattern.is(self.source_package_version_pattern))
            .select(SourcePackageTagRule::as_select())
            .get_result(connection)?;

        Ok(existing)
    }
}
