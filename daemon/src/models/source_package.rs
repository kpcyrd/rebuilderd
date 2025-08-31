use crate::schema::*;
use chrono::NaiveDateTime;
use diesel::dsl::update;
use diesel::prelude::*;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, Selectable, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = source_packages)]
pub struct SourcePackage {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub last_seen: NaiveDateTime,
    pub seen_in_last_sync: bool,
}

#[derive(Insertable, AsChangeset, PartialEq, Eq, Debug, Clone)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(treat_none_as_null = true)]
#[diesel(treat_none_as_default_value = false)]
#[diesel(table_name = source_packages)]
pub struct NewSourcePackage {
    pub name: String,
    pub version: String,
    pub distribution: String,
    pub release: Option<String>,
    pub component: Option<String>,
    pub last_seen: NaiveDateTime,
    pub seen_in_last_sync: bool,
}

impl NewSourcePackage {
    pub fn upsert(&self, connection: &mut SqliteConnection) -> Result<SourcePackage> {
        let existing = diesel::insert_into(source_packages::table)
            .values(self)
            .on_conflict_do_nothing() // TODO: two round trips here because Diesel doesn't support on_conflict() with no target, and we have uniqueness semantics for nullable columns
            .returning(SourcePackage::as_select())
            .get_result::<SourcePackage>(connection)
            .optional()?;

        if let Some(existing) = existing {
            return Ok(existing);
        }

        let updated = update(source_packages::table)
            .filter(
                source_packages::name
                    .is(&self.name)
                    .and(source_packages::version.is(&self.version))
                    .and(source_packages::distribution.is(&self.distribution)),
            )
            .filter(source_packages::release.is(&self.release))
            .filter(source_packages::component.is(&self.component))
            .set(self)
            .returning(SourcePackage::as_select())
            .get_result::<SourcePackage>(connection)?;

        Ok(updated)
    }
}
