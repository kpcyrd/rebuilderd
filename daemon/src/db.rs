use crate::code_migrations::code_migration;
use diesel::connection::{Instrumentation, SimpleConnection, TransactionManager};
use diesel::prelude::*;
use diesel::query_builder::{QueryFragment, QueryId};
use diesel::r2d2::{self, ConnectionManager};
use diesel::sql_query;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use rebuilderd_common::errors::*;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub type Pool = r2d2::Pool<ConnectionManager<SqliteConnectionWrap>>;

pub fn setup(url: &str) -> Result<SqliteConnection> {
    info!("Using database at {:?}", url);
    let mut connection = SqliteConnection::establish(url)?;

    while connection
        .has_pending_migration(MIGRATIONS)
        .map_err(|err| anyhow!("Failed to check for pending migrations: {err:#}"))?
    {
        let pending_migrations = connection
            .pending_migrations(MIGRATIONS)
            .map_err(|err| anyhow!("Failed to check for pending migrations: {err:#}"))?;

        let Some(next_migration) = pending_migrations.first() else {
            break;
        };

        let version = code_migration::run_code_backed_migration(&mut connection, next_migration)
            .map_err(|err| anyhow!("Failed to run pending migration: {err:#}"))?;

        info!("Applied database migration: {version}");
    }

    info!("reclaiming disk space (this might take a while)");
    sql_query("VACUUM;").execute(&mut connection)?;

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

impl std::convert::AsMut<SqliteConnection> for SqliteConnectionWrap {
    fn as_mut(&mut self) -> &mut SqliteConnection {
        &mut self.0
    }
}

impl diesel::r2d2::R2D2Connection for SqliteConnectionWrap {
    fn ping(&mut self) -> QueryResult<()> {
        self.0.ping()
    }

    fn is_broken(&mut self) -> bool {
        self.0.is_broken()
    }
}

impl diesel::connection::ConnectionSealed for SqliteConnectionWrap {}

impl diesel::connection::SimpleConnection for SqliteConnectionWrap {
    fn batch_execute(&mut self, query: &str) -> QueryResult<()> {
        self.0.batch_execute(query)
    }
}

impl Connection for SqliteConnectionWrap {
    type Backend = <SqliteConnection as Connection>::Backend;
    type TransactionManager = <SqliteConnection as Connection>::TransactionManager;

    fn establish(database_url: &str) -> ConnectionResult<Self> {
        let mut c = SqliteConnection::establish(database_url).map_err(|err| {
            warn!("establish returned error: {:?}", err);
            err
        })?;

        c.batch_execute(
            "
            PRAGMA busy_timeout = 10000;        -- sleep if the database is busy
            PRAGMA foreign_keys = ON;           -- enforce foreign keys
        ",
        )
        .map_err(|err| {
            warn!("executing pragmas for busy_timeout failed: {:?}", err);
            ConnectionError::CouldntSetupConfiguration(err)
        })?;

        c.batch_execute("
            PRAGMA journal_mode = WAL;          -- better write-concurrency
            PRAGMA synchronous = NORMAL;        -- fsync only in critical moments
            PRAGMA wal_autocheckpoint = 1000;   -- write WAL changes back every 1000 pages, for an in average 1MB WAL file. May affect readers if number is increased
            PRAGMA wal_checkpoint(TRUNCATE);    -- free some space by truncating possibly massive WAL files from the last run.
            PRAGMA cache_size = 134217728;      -- set disk cache size to 128MB
        ").map_err(|err| {
            warn!("executing pragmas for wall mode failed: {:?}", err);
            ConnectionError::CouldntSetupConfiguration(err)
        })?;

        Ok(Self(c))
    }

    fn execute_returning_count<T>(&mut self, source: &T) -> QueryResult<usize>
    where
        T: QueryFragment<Self::Backend> + QueryId,
    {
        self.0.execute_returning_count(source)
    }

    fn transaction_state(
        &mut self,
    ) -> &mut <Self::TransactionManager as TransactionManager<Self>>::TransactionStateData {
        self.0.transaction_state()
    }

    fn instrumentation(&mut self) -> &mut dyn Instrumentation {
        self.0.instrumentation()
    }

    fn set_instrumentation(&mut self, instrumentation: impl Instrumentation) {
        self.0.set_instrumentation(instrumentation)
    }
}
