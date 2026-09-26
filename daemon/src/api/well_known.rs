use crate::web;
use actix_web::{HttpRequest, HttpResponse, Responder, get};
use data_encoding::BASE64URL_NOPAD;
use in_toto::crypto::{PrivateKey, PublicKey, SignatureScheme};
use rebuilderd_common::errors::*;
use serde_json::json;
use std::sync::Arc;

/// Build a did:web DID document that lists the attestation signing key.
///
/// The verification method is named after the in-toto key id, so the `keyid`
/// of an attestation signature can be resolved to `<did>#<keyid>`.
///
/// See https://w3c-ccg.github.io/did-method-web/
fn did_document(host: &str, public_key: &PublicKey) -> Result<serde_json::Value> {
    // did:web requires the port separator to be percent-encoded
    let did = format!("did:web:{}", host.replace(':', "%3A"));
    // This is currently the only way to get the string out of in_toto::crypto::KeyId:
    // https://github.com/in-toto/in-toto-rs/issues/114
    let key_id: String = serde_json::from_value(serde_json::to_value(public_key.key_id())?)?;
    let did_with_key_id = format!("{did}#{key_id}");

    let (verification_methods, assertion_methods) = match public_key.scheme() {
        // Rebuilderd only generates ed25519 keys, an in-toto key could in theory have other schemes
        SignatureScheme::Ed25519 => (
            json!([{
                "id": did_with_key_id,
                "type": "JsonWebKey2020",
                "controller": did,
                "publicKeyJwk": {
                    "kty": "OKP",
                    "crv": "Ed25519",
                    "x": BASE64URL_NOPAD.encode(public_key.as_bytes()),
                },
            }]),
            json!([did_with_key_id]),
        ),
        _ => (json!([]), json!([])),
    };

    Ok(json!({
        "@context": [
            "https://www.w3.org/ns/did/v1",
            "https://w3id.org/security/suites/jws-2020/v1",
        ],
        "id": did,
        "verificationMethod": verification_methods,
        "assertionMethod": assertion_methods,
    }))
}

#[get("/.well-known/did.json")]
pub async fn get_did_document(
    request: HttpRequest,
    private_key: web::Data<Arc<PrivateKey>>,
) -> web::Result<impl Responder> {
    // We take the Host header from the incoming request to construct the did:web DID.
    // This is because a rebuilderd instance doesn't know it's own domain name or public port.
    // This is a user-provided value, but it's never signed, and the only way to get
    // a "weird response" is to send a "weird request", so I can't think of a way this could be a problem.
    let document = did_document(request.connection_info().host(), private_key.public())?;
    Ok(HttpResponse::Ok().json(document))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header;
    use actix_web::{App, test};
    use in_toto::crypto::{KeyType, SignatureScheme};

    fn private_key() -> PrivateKey {
        let privkey = PrivateKey::new(KeyType::Ed25519).unwrap();
        PrivateKey::from_pkcs8(&privkey, SignatureScheme::Ed25519).unwrap()
    }

    #[actix_web::test]
    async fn serves_signing_key() {
        let private_key = Arc::new(private_key());
        let public_key = private_key.public().clone();
        let key_id: String = serde_json::from_value(json!(public_key.key_id())).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(private_key))
                .service(get_did_document),
        )
        .await;
        let req = test::TestRequest::get()
            .uri("/.well-known/did.json")
            .insert_header((header::HOST, "rebuilder.example.com:8484"))
            .to_request();
        let document: serde_json::Value = test::call_and_read_body_json(&app, req).await;

        let did = "did:web:rebuilder.example.com%3A8484";
        let verification_method = format!("{did}#{key_id}");
        assert_eq!(document["id"], did);
        assert_eq!(document["verificationMethod"][0]["id"], verification_method);
        assert_eq!(document["verificationMethod"][0]["controller"], did);
        assert_eq!(document["assertionMethod"], json!([verification_method]));

        let x = document["verificationMethod"][0]["publicKeyJwk"]["x"]
            .as_str()
            .unwrap();
        assert_eq!(
            BASE64URL_NOPAD.decode(x.as_bytes()).unwrap(),
            public_key.as_bytes()
        );
    }
}
