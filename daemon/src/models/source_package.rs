use crate::schema::*;
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
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
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
}

impl NewSourcePackage {
    pub fn upsert(&self, connection: &mut SqliteConnection) -> Result<SourcePackage> {
        let result = diesel::insert_into(source_packages::table)
            .values(self)
            .on_conflict_do_nothing() // TODO: two round trips here because Diesel doesn't support on_conflict() with no target, and we have uniqueness semantics for nullable columns
            .returning(SourcePackage::as_select())
            .get_result::<SourcePackage>(connection)
            .optional()?;

        if let Some(result) = result {
            return Ok(result);
        }

        let mut sql = source_packages::table
            .filter(
                source_packages::name
                    .eq(&self.name)
                    .and(source_packages::version.eq(&self.version))
                    .and(source_packages::distribution.eq(&self.distribution)),
            )
            .into_boxed();

        if let Some(release) = &self.release {
            sql = sql.filter(source_packages::release.eq(release));
        } else {
            sql = sql.filter(source_packages::release.is_null());
        }

        if let Some(component) = &self.component {
            sql = sql.filter(source_packages::component.eq(component));
        } else {
            sql = sql.filter(source_packages::component.is_null());
        }

        let existing = sql.get_result::<SourcePackage>(connection)?;
        Ok(existing)
    }
}
