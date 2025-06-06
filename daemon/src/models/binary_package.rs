use crate::models::BuildInput;
use crate::models::SourcePackage;
use crate::schema::*;
use diesel::dsl;
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

diesel::alias!(crate::schema::rebuilds as r1: RebuildsAlias1, crate::schema::rebuilds as r2: RebuildsAlias2);

impl BinaryPackage {
    #[dsl::auto_type(no_type_alias)]
    pub fn filter_by<'a>(
        name: Option<&'a str>,
        distribution: Option<&'a str>,
        release: Option<&'a str>,
        component: Option<&'a str>,
        architecture: Option<&'a str>,
        status: Option<&'a str>,
    ) -> _ {
        let mut query = binary_packages::table
            .inner_join(source_packages::table)
            .inner_join(build_inputs::table)
            .left_join(r1.on(r1.field(rebuilds::build_input_id).eq(build_inputs::id)))
            .left_join(
                rebuild_artifacts::table.on(rebuild_artifacts::rebuild_id
                    .eq(r1.field(rebuilds::id))
                    .and(rebuild_artifacts::name.eq(binary_packages::name))),
            )
            .left_join(
                r2.on(r2.field(rebuilds::build_input_id).eq(build_inputs::id).and(
                    r1.field(rebuilds::built_at)
                        .lt(r2.field(rebuilds::built_at))
                        .or(r1.fields(
                            rebuilds::built_at
                                .eq(r2.field(rebuilds::built_at))
                                .and(r1.field(rebuilds::id).lt(r2.field(rebuilds::id))),
                        )),
                )),
            )
            .filter(r2.field(rebuilds::id).is_null())
            .into_boxed::<'a, diesel::sqlite::Sqlite>();

        if let Some(name) = name {
            query = query.filter(source_packages::name.eq(name));
        }

        if let Some(distribution) = distribution {
            query = query.filter(source_packages::distribution.eq(distribution));
        }

        if let Some(release) = release {
            query = query.filter(source_packages::release.eq(release));
        }

        if let Some(component) = component {
            query = query.filter(source_packages::release.eq(component));
        }

        if let Some(architecture) = architecture {
            query = query.filter(build_inputs::architecture.eq(architecture));
        }

        if let Some(status) = status {
            if status == "UNKWN" {
                query = query.filter(
                    r1.field(rebuilds::status)
                        .eq(status.to_string())
                        .or(r1.field(rebuilds::status).is_null()),
                );
            } else {
                query = query.filter(r1.field(rebuilds::status).eq(status.to_string()));
            }
        }

        query
    }
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
