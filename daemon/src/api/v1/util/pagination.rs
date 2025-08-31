use diesel::query_builder::{AstPass, Query, QueryFragment};
use diesel::sql_types::Integer;
use diesel::sqlite::Sqlite;
use diesel::{QueryId, QueryResult, RunQueryDsl, SqliteConnection};
use rebuilderd_common::api::v1::{Page, SortDirection};
use std::error::Error;
use std::fmt;
use std::fmt::Formatter;

#[derive(Debug, Clone)]
struct InvalidSortFieldError;

impl fmt::Display for InvalidSortFieldError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "invalid sort field")
    }
}

impl Error for InvalidSortFieldError {}

pub trait PaginateDsl: Sized {
    fn paginate(self, page: Page) -> Paginated<Self>;
}

impl<Q> PaginateDsl for Q {
    fn paginate(self, page: Page) -> Paginated<Self> {
        Paginated { query: self, page }
    }
}

#[derive(Debug, Clone, QueryId)]
pub struct Paginated<Q> {
    query: Q,
    page: Page,
}

impl<Q: Query> Query for Paginated<Q> {
    type SqlType = Q::SqlType;
}

impl<Q> RunQueryDsl<SqliteConnection> for Paginated<Q> {}

impl<Q> QueryFragment<Sqlite> for Paginated<Q>
where
    Q: QueryFragment<Sqlite>,
{
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.unsafe_to_cache_prepared();

        out.push_sql("WITH base_query AS (");
        self.query.walk_ast(out.reborrow())?;
        out.push_sql(")");
        out.push_sql("SELECT * FROM base_query ");

        if let Some(sort) = &self.page.sort {
            if !sort.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(diesel::result::Error::QueryBuilderError(Box::new(
                    InvalidSortFieldError,
                )));
            }

            let mut conditions: Vec<Box<dyn Fn(&mut AstPass<'_, 'b, Sqlite>) -> QueryResult<()>>> =
                Vec::new();

            if let Some(after) = &self.page.after {
                conditions.push(Box::new(|ast| {
                    let formatted = format!("(base_query.{field}, base_query.id) > ((SELECT base_query.{field} FROM base_query WHERE base_query.id = ", field = sort);
                    ast.push_sql(&formatted);
                    ast.push_bind_param::<Integer, _>(after)?;
                    ast.push_sql("), ");
                    ast.push_bind_param::<Integer, _>(after)?;
                    ast.push_sql(") ");

                    Ok(())
                }));
            }

            if let Some(before) = &self.page.before {
                conditions.push(Box::new(|ast| {
                    let formatted = format!("(base_query.{field}, base_query.id) < ((SELECT base_query.{field} FROM base_query WHERE base_query.id = ", field = sort);
                    ast.push_sql(&formatted);
                    ast.push_bind_param::<Integer, _>(before)?;
                    ast.push_sql("), ");
                    ast.push_bind_param::<Integer, _>(before)?;
                    ast.push_sql(") ");

                    Ok(())
                }));
            }

            let mut it = conditions.iter().peekable();
            if it.peek().is_some() {
                out.push_sql("WHERE");
                while let Some(condition) = it.next() {
                    condition(&mut out)?;
                    if it.peek().is_some() {
                        out.push_sql(" AND ")
                    }
                }
            }

            let direction = self
                .page
                .direction
                .clone()
                .unwrap_or(SortDirection::Ascending);

            let formatted = match direction {
                SortDirection::Ascending => format!(
                    "ORDER BY base_query.{field} ASC, base_query.id ASC ",
                    field = sort
                ),
                SortDirection::Descending => format!(
                    "ORDER BY base_query.{field} DESC, base_query.id DESC ",
                    field = sort
                ),
            };

            out.push_sql(&formatted);
        } else {
            let mut conditions: Vec<Box<dyn Fn(&mut AstPass<'_, 'b, Sqlite>) -> QueryResult<()>>> =
                Vec::new();

            if let Some(after) = &self.page.after {
                conditions.push(Box::new(|ast| {
                    ast.push_sql("base_query.id >");
                    ast.push_bind_param::<Integer, _>(after)
                }));
            }

            if let Some(before) = &self.page.before {
                conditions.push(Box::new(|ast| {
                    ast.push_sql("base_query.id <");
                    ast.push_bind_param::<Integer, _>(before)
                }));
            }

            let mut it = conditions.iter().peekable();
            if it.peek().is_some() {
                out.push_sql("WHERE ");
                while let Some(condition) = it.next() {
                    condition(&mut out)?;
                    if it.peek().is_some() {
                        out.push_sql(" AND ")
                    }
                }
            }

            let direction = self
                .page
                .direction
                .clone()
                .unwrap_or(SortDirection::Ascending);

            match direction {
                SortDirection::Ascending => out.push_sql("ORDER BY base_query.id ASC "),
                SortDirection::Descending => out.push_sql("ORDER BY base_query.id DESC "),
            }
        }

        out.push_sql("LIMIT ");
        if let Some(limit) = &self.page.limit {
            out.push_bind_param::<Integer, _>(limit)?;
        } else {
            out.push_sql("1000");
        }

        Ok(())
    }
}
