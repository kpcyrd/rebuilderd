use crate::models::BuildInput;
use crate::models::SourcePackage;
use crate::schema::*;
use diesel::prelude::*;
use diesel::upsert::excluded;
use rebuilderd_common::errors::*;

#[derive(
    Identifiable, Queryable, Selectable, Associations, AsChangeset, Clone, PartialEq, Eq, Debug,
)]
#[diesel(belongs_to(SourcePackage))]
#[diesel(belongs_to(BuildInput))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(table_name = binary_packages)]
pub struct BinaryPackage {
    pub id: i32,
    pub source_package_id: i32,
    pub build_input_id: i32,
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub artifact_url: String,
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(table_name = binary_packages)]
pub struct NewBinaryPackage {
    pub source_package_id: i32,
    pub build_input_id: i32,
    pub name: String,
    pub version: String,
    pub architecture: String,
    pub artifact_url: String,
}

impl NewBinaryPackage {
    pub fn upsert(&self, connection: &mut SqliteConnection) -> Result<BinaryPackage> {
        use crate::schema::binary_packages::*;

        let result = diesel::insert_into(table)
            .values(self)
            .on_conflict((
                source_package_id,
                build_input_id,
                name,
                version,
                architecture,
            ))
            .do_update()
            .set((
                source_package_id.eq(excluded(source_package_id)),
                build_input_id.eq(excluded(build_input_id)),
                name.eq(excluded(name)),
                version.eq(excluded(version)),
                architecture.eq(excluded(architecture)),
            ))
            .returning(BinaryPackage::as_select())
            .get_result::<BinaryPackage>(connection)?;

        Ok(result)
    }
}
