use crate::web;
use actix_web::{HttpRequest, HttpResponse, Responder, get};
use data_encoding::BASE64URL_NOPAD;
use in_toto::crypto::{PrivateKey, PublicKey};
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
    let key_id: String = serde_json::from_value(serde_json::to_value(public_key.key_id())?)?;
    let verification_method = format!("{did}#{key_id}");

    Ok(json!({
        "@context": [
            "https://www.w3.org/ns/did/v1",
            "https://w3id.org/security/suites/jws-2020/v1",
        ],
        "id": did,
        "verificationMethod": [{
            "id": verification_method,
            "type": "JsonWebKey2020",
            "controller": did,
            "publicKeyJwk": {
                "kty": "OKP",
                "crv": "Ed25519",
                "x": BASE64URL_NOPAD.encode(public_key.as_bytes()),
            },
        }],
        "assertionMethod": [verification_method],
    }))
}

#[get("/.well-known/did.json")]
pub async fn get_did_document(
    request: HttpRequest,
    private_key: web::Data<Arc<PrivateKey>>,
) -> web::Result<impl Responder> {
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
