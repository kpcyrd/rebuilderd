use actix_web::HttpRequest;
use chrono::Utc;
use crypto_secretbox::{
    AeadCore, KeyInit, XSalsa20Poly1305,
    aead::{Aead, OsRng},
};
use rebuilderd_common::errors::*;
use serde::{Deserialize, Serialize};

pub const COOKIE_NAME: &str = "auth";
pub const SESSION_TTL: chrono::Duration = chrono::Duration::hours(6);

const ENCODING: data_encoding::Encoding = data_encoding::BASE64URL_NOPAD;

pub struct Session {
    secret_key: crypto_secretbox::Key,
}

impl Session {
    pub fn new() -> Self {
        let secret_key = XSalsa20Poly1305::generate_key(&mut OsRng);
        Self { secret_key }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Vec<u8> {
        let cipher = XSalsa20Poly1305::new(&self.secret_key);
        let nonce = XSalsa20Poly1305::generate_nonce(&mut OsRng); // unique per message
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .expect("encryption failure!");
        [nonce.as_slice(), ciphertext.as_slice()].concat()
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> Option<Vec<u8>> {
        let (nonce_bytes, ciphertext) =
            ciphertext.split_at_checked(XSalsa20Poly1305::NONCE_SIZE)?;

        let nonce = crypto_secretbox::Nonce::from_slice(nonce_bytes);
        let cipher = XSalsa20Poly1305::new(&self.secret_key);

        cipher.decrypt(&nonce, ciphertext).ok()
    }

    pub fn encrypt_session(&self, session: &SessionData) -> Result<String> {
        let plaintext = serde_json::to_vec(session)?;
        let ciphertext = self.encrypt(&plaintext);
        Ok(ENCODING.encode(&ciphertext))
    }

    pub fn decrypt_session(&self, ciphertext: &str) -> Result<SessionData> {
        let ciphertext = ENCODING
            .decode(ciphertext.as_bytes())
            .with_context(|| anyhow!("Failed to decode session data"))?;

        let decrypted_bytes = self
            .decrypt(&ciphertext)
            .ok_or_else(|| anyhow!("Failed to decrypt session data"))?;

        let session: SessionData = serde_json::from_slice(&decrypted_bytes)
            .with_context(|| anyhow!("Failed to deserialize session data"))?;

        if session.is_expired() {
            return Err(anyhow!("Session has expired"));
        }

        Ok(session)
    }

    pub fn from_request(&self, req: &HttpRequest) -> Option<SessionData> {
        let cookie = req.cookie(COOKIE_NAME)?;
        self.decrypt_session(cookie.value()).ok()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionData {
    pub username: String,
    expires_at: chrono::DateTime<Utc>,
}

impl SessionData {
    pub fn new(username: String) -> Self {
        let expires_at = Utc::now() + SESSION_TTL;
        Self {
            username,
            expires_at,
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}
