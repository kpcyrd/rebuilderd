use chrono::NaiveDateTime;
use crate::schema::*;
use diesel::prelude::*;
use rebuilderd_common::PkgRelease;
use rebuilderd_common::errors::*;

#[derive(Identifiable, Queryable, AsChangeset, Clone, PartialEq, Eq, Debug)]
#[diesel(treat_none_as_null = true)]
#[diesel(table_name = packages)]
pub struct Package {
    pub id: i32,
    pub pkgbase_id: i32,
    pub name: String,
    pub version: String,
    pub status: String,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub artifact_url: String,
    pub build_id: Option<i32>,
    pub built_at: Option<NaiveDateTime>,
    pub has_diffoscope: bool,
    pub has_attestation: bool,
    pub checksum: Option<String>,
}

impl Package {
    pub fn get_id(my_id: i32, connection: &mut SqliteConnection) -> Result<Package> {
        use crate::schema::packages::dsl::*;
        let pkg = packages
            .filter(id.eq(my_id))
            .first::<Package>(connection)?;
        Ok(pkg)
    }

    pub fn get_by(my_name: &str, my_distro: &str, my_suite: &str, my_architecture: Option<&str>, connection: &mut SqliteConnection) -> Result<Vec<Package>> {
        use crate::schema::packages::dsl::*;
        let mut query = packages
            .filter(name.eq(my_name))
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .into_boxed();
        if let Some(my_architecture) = my_architecture {
            query = query.filter(architecture.eq(my_architecture));
        }
        let pkg = query.load::<Package>(connection)?;
        Ok(pkg)
    }

    pub fn get_by_api(pkg: &PkgRelease, connection: &mut SqliteConnection) -> Result<Package> {
        use crate::schema::packages::dsl::*;
        let pkg = packages
            .filter(name.eq(&pkg.name))
            .filter(version.eq(&pkg.version))
            .filter(distro.eq(&pkg.distro))
            .filter(suite.eq(&pkg.suite))
            .filter(architecture.eq(&pkg.architecture))
            .first::<Package>(connection)?;
        Ok(pkg)
    }

    pub fn list(connection: &mut SqliteConnection) -> Result<Vec<Package>> {
        use crate::schema::packages::dsl::*;
        let pkgs = packages
            .order_by((name, distro))
            .load::<Package>(connection)?;
        Ok(pkgs)
    }

    pub fn list_distro_suite(my_distro: &str, my_suite: &str, connection: &mut SqliteConnection) -> Result<Vec<Package>> {
        use crate::schema::packages::dsl::*;
        let pkgs = packages
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .load::<Package>(connection)?;
        Ok(pkgs)
    }

    pub fn update(&self, connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::packages::columns::*;
        diesel::update(packages::table.filter(id.eq(self.id)))
            .set(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn reset_status_for_requeued_list(pkgs: &[i32], connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::packages::columns::*;
        diesel::update(packages::table
                .filter(id.eq_any(pkgs))
            )
            .set((
                status.eq("UNKWN"),
                build_id.eq(None as Option<i32>),
            ))
            .execute(connection)?;
        Ok(())
    }

    pub fn delete(my_id: i32, connection: &mut SqliteConnection) -> Result<()> {
        use crate::schema::packages::dsl::*;
        diesel::delete(packages.filter(id.eq(my_id)))
            .execute(connection)?;
        Ok(())
    }

    pub fn most_recent_built_at(connection: &mut SqliteConnection) -> Result<Option<NaiveDateTime>> {
        use crate::schema::packages::dsl::*;

        let latest_built = packages.select(diesel::dsl::max(built_at)).first(connection)?;

        Ok(latest_built)
    }

    pub fn into_api_item(self) -> Result<PkgRelease> {
        Ok(PkgRelease {
            name: self.name,
            distro: self.distro,
            architecture: self.architecture,
            version: self.version,
            status: self.status.parse()?,
            suite: self.suite,
            artifact_url: self.artifact_url,
            build_id: self.build_id,
            built_at: self.built_at,
            has_diffoscope: self.has_diffoscope,
            has_attestation: self.has_attestation,
        })
    }
}

#[derive(Insertable, PartialEq, Eq, Debug, Clone)]
#[diesel(table_name = packages)]
pub struct NewPackage {
    pub pkgbase_id: i32,
    pub name: String,
    pub version: String,
    pub status: String,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub artifact_url: String,
    pub build_id: Option<i32>,
    pub built_at: Option<NaiveDateTime>,
    pub has_diffoscope: bool,
    pub has_attestation: bool,
    pub checksum: Option<String>,
}

impl NewPackage {
    pub fn insert(&self, connection: &mut SqliteConnection) -> Result<()> {
        diesel::insert_into(packages::table)
            .values(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn insert_batch(pkgs: &[NewPackage], connection: &mut SqliteConnection) -> Result<()> {
        diesel::insert_into(packages::table)
            .values(pkgs)
            .execute(connection)?;
        Ok(())
    }

    pub fn from_api(distro: String, pkgbase_id: i32, pkg: PkgRelease) -> NewPackage {
        NewPackage {
            pkgbase_id,
            name: pkg.name,
            version: pkg.version,
            status: pkg.status.to_string(),
            distro,
            suite: pkg.suite,
            architecture: pkg.architecture,
            artifact_url: pkg.artifact_url,
            build_id: None,
            built_at: None,
            has_diffoscope: false,
            has_attestation: false,
            checksum: None,
        }
    }
}
