use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use rebuilderd_common::errors::*;
use diesel::query_builder::QueryId;
use diesel::query_builder::QueryFragment;
use diesel::deserialize::QueryableByName;
use diesel::connection::SimpleConnection;
use diesel::query_builder::AsQuery;
use diesel::sql_types::HasSqlType;
use std::io;

embed_migrations!("migrations");

pub type Pool = r2d2::Pool<ConnectionManager<SqliteConnectionWrap>>;

pub fn setup(url: &str) -> Result<SqliteConnection> {
    let connection = SqliteConnection::establish(url)?;
    embedded_migrations::run_with_output(&connection, &mut io::stdout())?;
    Ok(connection)
}

pub fn setup_pool(url: &str) -> Result<Pool> {
    setup(url)?;

    let manager = ConnectionManager::<SqliteConnectionWrap>::new(url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .context("Failed to create pool")?;
    Ok(pool)
}

pub struct SqliteConnectionWrap(SqliteConnection);

impl AsRef<SqliteConnection> for SqliteConnectionWrap {
    fn as_ref(&self) -> &SqliteConnection {
        &self.0
    }
}

impl SimpleConnection for SqliteConnectionWrap {
    fn batch_execute(&self, query: &str) -> QueryResult<()> {
        self.0.batch_execute(query)
    }
}

impl Connection for SqliteConnectionWrap {
    type Backend = <SqliteConnection as Connection>::Backend;
    type TransactionManager = <SqliteConnection as Connection>::TransactionManager;

    fn establish(database_url: &str) -> ConnectionResult<Self> {
        let c = SqliteConnection::establish(database_url)?;
        // TODO: wal doesn't work yet
        // c.batch_execute("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON").unwrap();
        Ok(Self(c))
    }

    fn execute(&self, query: &str) -> QueryResult<usize> {
        self.0.execute(query)
    }

    fn query_by_index<T, U>(&self, source: T) -> QueryResult<Vec<U>>
        where
            T: AsQuery,
            T::Query: QueryFragment<Self::Backend> + QueryId,
            Self::Backend: HasSqlType<T::SqlType>,
            U: Queryable<T::SqlType, Self::Backend>,
    {
        self.0.query_by_index(source)
    }

    fn query_by_name<T, U>(&self, source: &T) -> QueryResult<Vec<U>>
        where
        T: QueryFragment<Self::Backend> + QueryId,
        U: QueryableByName<Self::Backend>,
    {
        self.0.query_by_name(source)
    }

    fn execute_returning_count<T>(&self, source: &T) -> QueryResult<usize>
        where
        T: QueryFragment<Self::Backend> + QueryId,
    {
        self.0.execute_returning_count(source)
    }

    fn transaction_manager(&self) -> &Self::TransactionManager {
        self.0.transaction_manager()
    }
}
