use in_toto::crypto::{KeyType, PrivateKey, SignatureScheme};
use rebuilderd_common::api::Client;
use rebuilderd_common::config::ConfigFile;
use rebuilderd_common::errors::*;
use std::fs;
use std::path::Path;
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::io::prelude::*;

pub struct Profile {
    pub pubkey: String,
    pub privkey: PrivateKey,
}

impl Profile {
    pub fn new_client(&self, config: ConfigFile, endpoint: String, signup_secret: Option<String>, auth_cookie: Option<String>) -> Client {
        let mut client = Client::new(config, Some(endpoint));
        client.worker_key(self.pubkey.clone());
        if let Some(signup_secret) = signup_secret {
            client.signup_secret(signup_secret);
        } else if let Some(auth_cookie) = auth_cookie {
            client.auth_cookie(auth_cookie);
        }
        client
    }
}

#[inline]
pub fn load() -> Result<Profile> {
    load_key("rebuilder.key")
}

fn load_key<P: AsRef<Path>>(path: P) -> Result<Profile> {
    let path = path.as_ref();

    let privkey = if path.exists() {
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

    let pk = privkey.public();
    let pubkey = base64::encode(pk.as_bytes());

    Ok(Profile {
        pubkey,
        privkey,
    })
}
