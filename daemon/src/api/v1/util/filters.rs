use crate::schema::source_packages;
use diesel::expression::AsExpression;
use diesel::query_dsl::filter_dsl::FilterDsl;
use diesel::sql_types::{Bool, Text};
use diesel::sqlite::Sqlite;
use diesel::{BoxableExpression, Column, Expression, SelectableExpression};
use diesel::{ExpressionMethods, SqliteExpressionMethods};
use rebuilderd_common::api::v1::{FreshnessFilter, IdentityFilter, OriginFilter};

pub trait DieselOriginFilter<'a> {
    fn filter<Q>(&'a self, sql: Q) -> Q
    where
        Q: FilterDsl<diesel::dsl::Eq<source_packages::distribution, &'a String>, Output = Q>,
        Q: FilterDsl<diesel::dsl::Eq<source_packages::release, &'a String>, Output = Q>,
        Q: FilterDsl<diesel::dsl::Eq<source_packages::component, &'a String>, Output = Q>;
}

impl<'a> DieselOriginFilter<'a> for OriginFilter {
    fn filter<Q>(&'a self, mut sql: Q) -> Q
    where
        Q: FilterDsl<diesel::dsl::Eq<source_packages::distribution, &'a String>, Output = Q>,
        Q: FilterDsl<diesel::dsl::Eq<source_packages::release, &'a String>, Output = Q>,
        Q: FilterDsl<diesel::dsl::Eq<source_packages::component, &'a String>, Output = Q>,
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

        sql
    }
}

pub trait DieselIdentityFilter<'a> {
    fn filter<Q, N, V>(&'a self, sql: Q, name_column: N, version_column: V) -> Q
    where
        N: Column + Expression<SqlType = Text>,
        V: Column + Expression<SqlType = Text>,
        Q: FilterDsl<diesel::dsl::Eq<N, &'a String>, Output = Q>,
        Q: FilterDsl<diesel::dsl::Eq<V, &'a String>, Output = Q>;
}

impl<'a> DieselIdentityFilter<'a> for IdentityFilter {
    fn filter<Q, N, V>(&'a self, mut sql: Q, name_column: N, version_column: V) -> Q
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

pub trait IntoFilter<QS, DB> {
    type SqlType;

    fn into_filter(self) -> Box<dyn BoxableExpression<QS, DB, SqlType = Self::SqlType>>;
}

impl<T> IntoFilter<T, Sqlite> for FreshnessFilter
where
    source_packages::seen_in_last_sync: SelectableExpression<T>,
{
    type SqlType = Bool;

    fn into_filter(self) -> Box<dyn BoxableExpression<T, Sqlite, SqlType = Self::SqlType>> {
        match self.seen_only {
            Some(seen_only) => Box::new(source_packages::seen_in_last_sync.is(seen_only)),
            None => Box::new(AsExpression::<Bool>::as_expression(true)),
        }
    }
}
