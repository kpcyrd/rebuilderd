use data_encoding::BASE64;
use in_toto::crypto::{KeyType, PrivateKey, SignatureScheme};
use rebuilderd_common::api::Client;
use rebuilderd_common::config::ConfigFile;
use rebuilderd_common::errors::*;
use std::fs;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::io::ErrorKind;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub struct Profile {
    pub pubkey: String,
    pub privkey: PrivateKey,
}

impl Profile {
    pub fn new_client(
        &self,
        config: ConfigFile,
        endpoint: String,
        signup_secret: Option<String>,
        auth_cookie: Option<String>,
    ) -> Result<Client> {
        let mut client = Client::new(config, Some(endpoint))?;
        client.worker_key(self.pubkey.clone());
        if let Some(signup_secret) = signup_secret {
            client.signup_secret(signup_secret);
        } else if let Some(auth_cookie) = auth_cookie {
            client.auth_cookie(auth_cookie);
        }
        Ok(client)
    }
}

#[inline]
pub fn load() -> Result<Profile> {
    match fs::remove_file("rebuilder.key") {
        Ok(_) => info!("Deleted old v1 worker key"),
        Err(err) if err.kind() == ErrorKind::NotFound => (),
        Err(err) => warn!("Failed to delete old v1 worker key: {:#}", err),
    }

    load_key("rebuilder.v2.key")
}

fn load_key<P: AsRef<Path>>(path: P) -> Result<Profile> {
    let path = path.as_ref();

    let privkey = match OpenOptions::new()
        .mode(0o640)
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            // file didn't exist yet, generate new key
            let secret = PrivateKey::new(KeyType::Ed25519)?;
            file.write_all(&secret[..])?;
            secret
        }
        Err(_err) => {
            // assume the file already exists, try reading the content
            fs::read(path)?
        }
    };

    let privkey = PrivateKey::from_pkcs8(&privkey, SignatureScheme::Ed25519)?;
    let pubkey = BASE64.encode(privkey.public().as_bytes());

    Ok(Profile { pubkey, privkey })
}
