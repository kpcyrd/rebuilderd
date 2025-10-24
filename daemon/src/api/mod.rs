use crate::web;
use actix_web::http::header::{AcceptEncoding, ContentEncoding, Encoding, Header};
use actix_web::{HttpRequest, HttpResponse};
use rebuilderd_common::errors::{format_err, Context, Error};
use rebuilderd_common::utils::{is_zstd_compressed, zstd_decompress};

pub mod v0;
pub mod v1;

const DEFAULT_QUEUE_PRIORITY: i32 = 1;

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
