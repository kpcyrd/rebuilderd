use rebuilderd_common::api::Client;
use rebuilderd_common::config::ConfigFile;
use rebuilderd_common::errors::*;
use std::fs;
use std::path::Path;
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::io::prelude::*;
use in_toto::crypto::{PrivateKey, KeyType, SignatureScheme};
use serde_json;

pub struct Profile {
    key: String,
}

impl Profile {
    pub fn new_client(&self, config: ConfigFile, endpoint: String, signup_secret: Option<String>, auth_cookie: Option<String>) -> Client {
        let mut client = Client::new(config, Some(endpoint));
        client.worker_key(self.key.clone());
        if let Some(signup_secret) = signup_secret {
            client.signup_secret(signup_secret);
        } else if let Some(auth_cookie) = auth_cookie {
            client.auth_cookie(auth_cookie);
        }
        client
    }
}

pub fn load() -> Result<Profile> {
    let key = load_key("rebuilder.key")?;

    Ok(Profile {
        key,
    })
}

fn load_key<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();

    let sk = if path.exists() {
        let content = fs::read(path)?;
        PrivateKey::from_pkcs8(&content, SignatureScheme::Ed25519)?
    } else {
        let sk = PrivateKey::new(KeyType::Ed25519)?;
        let mut file = OpenOptions::new()
            .mode(0o640)
            .write(true)
            .create(true)
            .open(path)?;
        file.write_all(&sk[..])?;

        PrivateKey::from_pkcs8(&sk, SignatureScheme::Ed25519)?
    };

    let pk = sk.public();
    let key  = serde_json::to_value(&pk)?;
    Ok(key.to_string())
}

pub fn load_signing_key<P: AsRef<Path>>(path: P) -> Result<PrivateKey> {
    let path = path.as_ref();

    let sk = if path.exists() {
        let content = fs::read(path)?;
        PrivateKey::from_pkcs8(&content, SignatureScheme::Ed25519)?
    } else {
        let sk = PrivateKey::new(KeyType::Ed25519)?;
        let mut file = OpenOptions::new()
            .mode(0o640)
            .write(true)
            .create(true)
            .open(path)?;
        file.write_all(&sk[..])?;

        PrivateKey::from_pkcs8(&sk, SignatureScheme::Ed25519)?
    };

    Ok(sk)
}
