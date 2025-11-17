use crate::code_migrations::code_migration::CodeMigration;
use diesel::migration::Migration;
use diesel::sql_types::Binary;
use diesel::sqlite::Sqlite;
use diesel::{RunQueryDsl, SqliteConnection, define_sql_function, sql_query};
use log::info;

pub struct CompressLogsMigration;

define_sql_function! {
    fn zstd_compress(data: Binary) -> Binary;
}

define_sql_function! {
    fn zstd_decompress(data: Binary) -> Binary;
}

impl CodeMigration for CompressLogsMigration {
    fn prepare(
        &self,
        connection: &mut SqliteConnection,
        _: &dyn Migration<Sqlite>,
    ) -> diesel::migration::Result<()> {
        zstd_compress_utils::register_impl(connection, |data: Vec<u8>| {
            zstd::encode_all(&data[..], 11).unwrap()
        })?;
        zstd_decompress_utils::register_impl(connection, |data: Vec<u8>| {
            zstd::decode_all(&data[..]).unwrap()
        })?;

        Ok(())
    }

    fn post_up(
        &self,
        connection: &mut SqliteConnection,
        _: &dyn Migration<Sqlite>,
    ) -> diesel::migration::Result<()> {
        info!("compressing build logs (this might take a while)");
        sql_query("UPDATE builds SET build_log = zstd_compress(build_log);").execute(connection)?;
        sql_query("UPDATE builds SET diffoscope = zstd_compress(diffoscope) WHERE diffoscope IS NOT NULL;")
            .execute(connection)?;
        sql_query("UPDATE builds SET attestation = zstd_compress(attestation) WHERE attestation IS NOT NULL;")
            .execute(connection)?;

        Ok(())
    }

    fn pre_down(
        &self,
        connection: &mut SqliteConnection,
        _: &dyn Migration<Sqlite>,
    ) -> diesel::migration::Result<()> {
        info!("decompressing build logs (this might take a while)");
        sql_query("UPDATE builds SET build_log = zstd_decompress(build_log);")
            .execute(connection)?;
        sql_query("UPDATE builds SET diffoscope = zstd_decompress(diffoscope) WHERE diffoscope IS NOT NULL;")
            .execute(connection)?;
        sql_query("UPDATE builds SET attestation = zstd_decompress(attestation) WHERE attestation IS NOT NULL;")
            .execute(connection)?;

        Ok(())
    }
}
