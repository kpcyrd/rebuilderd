use crate::diesel::ExpressionMethods;
use crate::schema::{binary_packages, build_inputs, source_packages};
use diesel::query_dsl::filter_dsl::FilterDsl;
use diesel::sql_types::Text;
use diesel::{Column, Expression};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OriginFilter {
    pub distribution: Option<String>,
    pub release: Option<String>,
    pub component: Option<String>,
    pub architecture: Option<String>,
}

impl<'a> OriginFilter {
    pub fn filter<Q, A>(&'a self, mut sql: Q, architecture_column: A) -> Q
    where
        A: OriginFilterColumn + Expression<SqlType = Text>,
        Q: FilterDsl<diesel::dsl::Eq<source_packages::distribution, &'a String>, Output = Q>,
        Q: FilterDsl<diesel::dsl::Eq<source_packages::release, &'a String>, Output = Q>,
        Q: FilterDsl<diesel::dsl::Eq<source_packages::component, &'a String>, Output = Q>,
        Q: FilterDsl<diesel::dsl::Eq<A, &'a String>, Output = Q>,
    {
        if let Some(distribution) = &self.distribution {
            sql = sql.filter(source_packages::distribution.eq(distribution));
        }

        if let Some(release) = &self.release {
            sql = sql.filter(source_packages::release.eq(release));
        }

        if let Some(component) = &self.component {
            sql = sql.filter(source_packages::component.eq(component));
        }

        if let Some(architecture) = &self.architecture {
            sql = sql.filter(architecture_column.eq(architecture));
        }

        sql
    }
}

pub trait OriginFilterColumn: Column {}

impl OriginFilterColumn for source_packages::distribution {}
impl OriginFilterColumn for source_packages::release {}
impl OriginFilterColumn for source_packages::component {}
impl OriginFilterColumn for build_inputs::architecture {}
impl OriginFilterColumn for binary_packages::architecture {}

#[derive(Debug, Deserialize)]
pub struct IdentityFilter {
    pub name: Option<String>,
    pub version: Option<String>,
}

impl<'a> IdentityFilter {
    pub fn filter<Q, N, V>(&'a self, mut sql: Q, name_column: N, version_column: V) -> Q
    where
        N: Column + Expression<SqlType = Text>,
        V: Column + Expression<SqlType = Text>,
        Q: FilterDsl<diesel::dsl::Eq<N, &'a String>, Output = Q>,
        Q: FilterDsl<diesel::dsl::Eq<V, &'a String>, Output = Q>,
    {
        if let Some(name) = &self.name {
            sql = sql.filter(name_column.eq(name));
        }

        if let Some(version) = &self.version {
            sql = sql.filter(version_column.eq(version));
        }

        sql
    }
}
