use crate::web;
use actix_web::http::header::{AcceptEncoding, ContentEncoding, Encoding, Header};
use actix_web::{HttpRequest, HttpResponse};
use rebuilderd_common::errors::{Context, Error, format_err};
use rebuilderd_common::utils::{is_zstd_compressed, zstd_decompress};

pub mod v0;
pub mod v1;
pub mod well_known;

/// Decompressing yields to the executor, so callers must release their database connection first.
pub async fn forward_compressed_data(
    request: HttpRequest,
    content_type: &str,
    data: Vec<u8>,
) -> web::Result<HttpResponse> {
    let mut builder = HttpResponse::Ok();

    builder
        .content_type(content_type)
        .append_header(("X-Content-Type-Options", "nosniff"))
        .append_header(("Content-Security-Policy", "default-src 'none'"));

    if is_zstd_compressed(data.as_slice()) {
        let client_supports_zstd = AcceptEncoding::parse(&request)
            .ok()
            .and_then(|a| a.negotiate([Encoding::zstd()].iter()))
            .map(|e| e == Encoding::zstd())
            .unwrap_or(false);

        if client_supports_zstd {
            builder.insert_header(ContentEncoding::Zstd);

            let resp = builder.body(data);
            Ok(resp)
        } else {
            let decoded_log = zstd_decompress(data.as_slice())
                .await
                .map_err(Error::from)?;

            let resp = builder.body(decoded_log);
            Ok(resp)
        }
    } else {
        let resp = builder.body(data);
        Ok(resp)
    }
}

pub fn header<'a>(req: &'a HttpRequest, key: &str) -> rebuilderd_common::errors::Result<&'a str> {
    let value = req
        .headers()
        .get(key)
        .ok_or_else(|| format_err!("Missing header"))?
        .to_str()
        .context("Failed to decode header value")?;

    Ok(value)
}

#[cfg(test)]
mod tests {
    use crate::db::{self, SqliteConnectionWrap};
    use crate::{api, web};
    use actix_web::web::scope;
    use actix_web::{App, test};
    use data_encoding::HEXLOWER;
    use diesel::connection::SimpleConnection;
    use diesel::r2d2::{self, ConnectionManager};
    use rebuilderd_common::utils::zstd_compress;
    use std::time::Duration;

    /// `pool.get()` blocks the worker thread, so a handler suspended while holding the only
    /// connection can't resume to release it.
    #[actix_web::test]
    async fn decompressing_does_not_hold_database_connection() {
        let dir = tempfile::tempdir().unwrap();
        let url = dir.path().join("db.sqlite");
        let url = url.to_str().unwrap();
        db::setup(url).unwrap();
        let pool = r2d2::Pool::builder()
            .max_size(1)
            .connection_timeout(Duration::from_secs(1))
            .build(ConnectionManager::<SqliteConnectionWrap>::new(url))
            .unwrap();

        let data = HEXLOWER.encode(&zstd_compress("data".as_bytes()).await.unwrap());
        pool.get()
            .unwrap()
            .batch_execute(&format!(
                "
                INSERT INTO source_packages (id, name, version, distribution, last_seen, seen_in_last_sync)
                    VALUES (1, 'foo', '1', 'archlinux', '2026-01-01 00:00:00', 1);
                INSERT INTO build_inputs (id, source_package_id, url, backend, architecture, retries)
                    VALUES (1, 1, 'https://example.com', 'archlinux', 'x86_64', 0);
                INSERT INTO build_logs (id, build_log) VALUES (1, X'{data}');
                INSERT INTO diffoscope_logs (id, diffoscope_log) VALUES (1, X'{data}');
                INSERT INTO rebuilds (id, build_input_id, built_at, build_log_id)
                    VALUES (1, 1, '2026-01-01 00:00:00', 1);
                INSERT INTO rebuild_artifacts (id, rebuild_id, name, diffoscope_log_id)
                    VALUES (1, 1, 'foo', 1);
                "
            ))
            .unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(pool))
                .service(
                    scope("/v0")
                        .service(api::v0::get_build_log)
                        .service(api::v0::get_diffoscope),
                )
                .service(
                    scope("/v1/builds")
                        .service(api::v1::get_build_log)
                        .service(api::v1::get_build_artifact_diffoscope),
                ),
        )
        .await;

        for uri in [
            "/v0/builds/1/log",
            "/v0/builds/1/diffoscope",
            "/v1/builds/1/log",
            "/v1/builds/1/artifacts/1/diffoscope",
        ] {
            // no accept-encoding header, so the handlers have to decompress their response
            let first = test::call_service(&app, test::TestRequest::get().uri(uri).to_request());
            let second = test::call_service(&app, test::TestRequest::get().uri(uri).to_request());
            let (first, second) = tokio::join!(first, second);
            assert!(second.status().is_success(), "{uri}: {}", second.status());
            assert_eq!(test::read_body(first).await, "data", "{uri}");
        }
    }
}
