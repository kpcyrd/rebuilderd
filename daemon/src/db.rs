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

impl SqliteConnectionWrap {
    fn establish_internal(database_url: &str) -> ConnectionResult<Self> {
        let c = SqliteConnection::establish(database_url)?;

        c.batch_execute("
            PRAGMA journal_mode = WAL;          -- better write-concurrency
            PRAGMA synchronous = NORMAL;        -- fsync only in critical moments
            PRAGMA wal_autocheckpoint = 1000;   -- write WAL changes back every 1000 pages, for an in average 1MB WAL file. May affect readers if number is increased
            PRAGMA wal_checkpoint(TRUNCATE);    -- free some space by truncating possibly massive WAL files from the last run.
            PRAGMA busy_timeout = 250;          -- sleep if the database is busy
            PRAGMA foreign_keys = ON;           -- enforce foreign keys
        ").map_err(ConnectionError::CouldntSetupConfiguration)?;

        Ok(Self(c))
    }
}

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
        // TODO: setting up an r2d2 pool shouldn't be this difficult
        for i in 0..3 {
            let result = Self::establish_internal(database_url);
            if result.is_ok() || i == 2 {
                return result;
            }
        }
        unreachable!()
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
