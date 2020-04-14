use crate::schema::*;
use rebuilderd_common::Status;
use rebuilderd_common::errors::*;
use diesel::prelude::*;
use rebuilderd_common::PkgRelease;
use rebuilderd_common::Distro;

#[derive(Identifiable, Queryable, AsChangeset, Clone, PartialEq, Debug)]
#[table_name="packages"]
pub struct Package {
    pub id: i32,
    pub name: String,
    pub version: String,
    pub status: String,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub url: String,
}

impl Package {
    pub fn get_id(my_id: i32, connection: &SqliteConnection) -> Result<Package> {
        use crate::schema::packages::dsl::*;
        let pkg = packages
            .filter(id.eq(my_id))
            .first::<Package>(connection)?;
        Ok(pkg)
    }

    pub fn get_by(my_name: &str, my_distro: &str, my_suite: &str, my_architecture: Option<&str>, connection: &SqliteConnection) -> Result<Vec<Package>> {
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

    pub fn get_by_api(pkg: &PkgRelease, connection: &SqliteConnection) -> Result<Package> {
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

    pub fn list(connection: &SqliteConnection) -> Result<Vec<Package>> {
        use crate::schema::packages::dsl::*;
        let pkgs = packages
            .order_by((name, distro))
            .load::<Package>(connection)?;
        Ok(pkgs)
    }

    pub fn list_distro_suite_architecture(my_distro: &str, my_suite: &str, my_architecture: &str, connection: &SqliteConnection) -> Result<Vec<Package>> {
        use crate::schema::packages::dsl::*;
        let pkgs = packages
            .filter(distro.eq(my_distro))
            .filter(suite.eq(my_suite))
            .filter(architecture.eq(my_architecture))
            .load::<Package>(connection)?;
        Ok(pkgs)
    }

    // when updating the verify status, use a custom query that enforces a version match
    pub fn update(&self, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::packages::columns::*;
        diesel::update(packages::table.filter(id.eq(self.id)))
            .set(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn update_status_safely(&mut self, my_status: Status, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::packages::columns::*;
        self.status = my_status.to_string();
        diesel::update(packages::table
                .filter(id.eq(self.id))
                .filter(version.eq(&self.version))
            )
            .set(&*self)
            .execute(connection)?;
        Ok(())
    }

    pub fn delete(my_id: i32, connection: &SqliteConnection) -> Result<()> {
        use crate::schema::packages::dsl::*;
        diesel::delete(packages.filter(id.eq(my_id)))
            .execute(connection)?;
        Ok(())
    }

    pub fn into_api_item(self) -> Result<PkgRelease> {
        Ok(PkgRelease {
            name: self.name,
            distro: self.distro,
            architecture: self.architecture,
            version: self.version,
            status: self.status.parse()?,
            suite: self.suite,
            url: self.url,
        })
    }
}

#[derive(Insertable, PartialEq, Debug, Clone)]
#[table_name="packages"]
pub struct NewPackage {
    pub name: String,
    pub version: String,
    pub status: String,
    pub distro: String,
    pub suite: String,
    pub architecture: String,
    pub url: String,
}

impl NewPackage {
    pub fn insert(&self, connection: &SqliteConnection) -> Result<()> {
        diesel::insert_into(packages::table)
            .values(self)
            .execute(connection)?;
        Ok(())
    }

    pub fn insert_batch(pkgs: &[NewPackage], connection: &SqliteConnection) -> Result<()> {
        diesel::insert_into(packages::table)
            .values(pkgs)
            .execute(connection)?;
        Ok(())
    }

    pub fn from_api(distro: Distro, pkg: PkgRelease) -> NewPackage {
        NewPackage {
            name: pkg.name,
            version: pkg.version,
            status: pkg.status.to_string(),
            distro: distro.to_string(),
            suite: pkg.suite,
            architecture: pkg.architecture,
            url: pkg.url,
        }
    }
}
