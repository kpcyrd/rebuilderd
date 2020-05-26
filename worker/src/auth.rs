use rebuilderd_common::api::Client;
use rebuilderd_common::config::ConfigFile;
use rebuilderd_common::errors::*;
use sodiumoxide::crypto::sign;
use std::fs;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub struct Profile {
    key: String,
}

impl Profile {
    pub fn new_client(
        &self,
        config: ConfigFile,
        endpoint: String,
        signup_secret: Option<String>,
        auth_cookie: Option<String>,
    ) -> Client {
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

    Ok(Profile { key })
}

fn load_key<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();

    let sk = if path.exists() {
        let content = fs::read(path)?;
        sign::SecretKey::from_slice(&content)
            .ok_or_else(|| format_err!("failed to load secret key"))?
    } else {
        let (_, sk) = sign::gen_keypair();

        let mut file = OpenOptions::new()
            .mode(0o640)
            .write(true)
            .create(true)
            .open(path)?;
        file.write_all(&sk[..])?;

        sk
    };

    let pk = sk.public_key();
    let key = base64::encode(&pk[..]);
    Ok(key)
}
