use crate::schema::source_packages;
use diesel::backend::Backend;
use diesel::expression::is_aggregate::No;
use diesel::expression::{AsExpression, ValidGrouping};
use diesel::query_builder::QueryFragment;
use diesel::sql_types::{Bool, Text};
use diesel::sqlite::Sqlite;
use diesel::{BoolExpressionMethods, BoxableExpression, Expression, SelectableExpression};
use diesel::{ExpressionMethods, SqliteExpressionMethods, TextExpressionMethods};
use rebuilderd_common::api::v1::{FreshnessFilter, IdentityFilter, OriginFilter};

pub trait IntoIdentityFilter<QS, DB>
where
    DB: Backend,
{
    type SqlType;

    type Output;

    fn into_filter<NameColumn, VersionColumn>(
        self,
        name_column: NameColumn,
        version_column: VersionColumn,
    ) -> Self::Output
    where
        NameColumn: SelectableExpression<QS>
            + Expression<SqlType = Text>
            + QueryFragment<DB>
            + ValidGrouping<(), IsAggregate = No>
            + ExpressionMethods
            + Send
            + 'static,
        VersionColumn: SelectableExpression<QS>
            + Expression<SqlType = Text>
            + QueryFragment<DB>
            + ValidGrouping<(), IsAggregate = No>
            + ExpressionMethods
            + Send
            + 'static;
}

impl<T: 'static> IntoIdentityFilter<T, Sqlite> for IdentityFilter {
    type SqlType = Bool;
    type Output = Box<dyn BoxableExpression<T, Sqlite, SqlType = Self::SqlType>>;

    fn into_filter<NameColumn, VersionColumn>(
        self,
        name_column: NameColumn,
        version_column: VersionColumn,
    ) -> Self::Output
    where
        NameColumn: SelectableExpression<T>
            + Expression<SqlType = Text>
            + QueryFragment<Sqlite>
            + ValidGrouping<(), IsAggregate = No>
            + ExpressionMethods
            + SqliteExpressionMethods
            + Send
            + 'static,
        VersionColumn: SelectableExpression<T>
            + Expression<SqlType = Text>
            + QueryFragment<Sqlite>
            + ValidGrouping<(), IsAggregate = No>
            + ExpressionMethods
            + Send
            + 'static,
    {
        // If both name and name_starts_with are set, name takes precedence
        let name_is: Self::Output = match (self.name, self.name_starts_with) {
            (Some(name), _) => {
                // Exact match for name
                Box::new(name_column.is(name))
            }
            (None, Some(prefix)) => {
                // LIKE pattern for name_starts_with - append % to the prefix
                let pattern = format!("{}%", prefix);
                Box::new(name_column.like(pattern))
            }
            (None, None) => Box::new(AsExpression::<Bool>::as_expression(true)),
        };

        let version_is: Self::Output = match self.version {
            Some(version) => Box::new(version_column.is(version)),
            None => Box::new(AsExpression::<Bool>::as_expression(true)),
        };

        Box::new(name_is.and(version_is))
    }
}

pub trait IntoOriginFilter<QS, DB>
where
    DB: Backend,
{
    type SqlType;

    type Output;

    fn into_filter<ArchitectureColumn>(
        self,
        architecture_column: ArchitectureColumn,
    ) -> Self::Output
    where
        ArchitectureColumn: SelectableExpression<QS>
            + Expression<SqlType = Text>
            + QueryFragment<DB>
            + ValidGrouping<(), IsAggregate = No>
            + ExpressionMethods
            + Send
            + 'static;
}

impl<T: 'static> IntoOriginFilter<T, Sqlite> for OriginFilter
where
    source_packages::distribution: SelectableExpression<T>,
    source_packages::release: SelectableExpression<T>,
    source_packages::component: SelectableExpression<T>,
{
    type SqlType = Bool;

    type Output = Box<dyn BoxableExpression<T, Sqlite, SqlType = Self::SqlType>>;

    fn into_filter<ArchitectureColumn>(
        self,
        architecture_column: ArchitectureColumn,
    ) -> Self::Output
    where
        ArchitectureColumn: SelectableExpression<T>
            + Expression<SqlType = Text>
            + QueryFragment<Sqlite>
            + ValidGrouping<(), IsAggregate = No>
            + ExpressionMethods
            + Send
            + SqliteExpressionMethods
            + 'static,
    {
        let distribution_is: Self::Output = match self.distribution {
            Some(distribution) => Box::new(source_packages::distribution.is(distribution)),
            None => Box::new(AsExpression::<Bool>::as_expression(true)),
        };

        let release_is: Self::Output = match self.release {
            Some(release) if !release.is_empty() => Box::new(source_packages::release.is(release)),
            Some(_) => Box::new(source_packages::release.is_null()), // Empty string means NULL
            None => Box::new(AsExpression::<Bool>::as_expression(true)),
        };

        let component_is: Self::Output = match self.component {
            Some(component) if !component.is_empty() => {
                Box::new(source_packages::component.is(component))
            }
            Some(_) => Box::new(source_packages::component.is_null()), // Empty string means NULL
            None => Box::new(AsExpression::<Bool>::as_expression(true)),
        };

        let architecture_is: Self::Output = match self.architecture {
            Some(architecture) => Box::new(architecture_column.is(architecture)),
            None => Box::new(AsExpression::<Bool>::as_expression(true)),
        };

        Box::new(
            distribution_is
                .and(release_is)
                .and(component_is)
                .and(architecture_is),
        )
    }
}

pub trait IntoFilter<QS, DB>
where
    DB: Backend,
{
    type SqlType;

    type Output;

    fn into_filter(self) -> Self::Output;
}

impl<T: 'static> IntoFilter<T, Sqlite> for FreshnessFilter
where
    source_packages::seen_in_last_sync: SelectableExpression<T>,
{
    type SqlType = Bool;

    type Output = Box<dyn BoxableExpression<T, Sqlite, SqlType = Self::SqlType>>;

    fn into_filter(self) -> Self::Output {
        match self.seen_only {
            Some(seen_only) => Box::new(source_packages::seen_in_last_sync.is(seen_only)),
            None => Box::new(AsExpression::<Bool>::as_expression(true)),
        }
    }
}
