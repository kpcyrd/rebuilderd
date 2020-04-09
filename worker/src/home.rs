use rebuilderd_common::errors::*;
use rebuilderd_common::api::Client;
use std::fs;
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::io::prelude::*;
use sodiumoxide::crypto::sign;

pub struct Profile {
    key: String,
}

impl Profile {
    pub fn new_client(&self, endpoint: String) -> Client {
        let mut client = Client::new(endpoint);
        client.worker_key(self.key.clone());
        client
    }
}

pub fn load(dir: Option<PathBuf>) -> Result<Profile> {
    let dir = if let Some(dir) = dir {
        dir
    } else {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| format_err!("Failed to find data directory"))?;
        data_dir.join("rebuilderd-worker")
    };
    info!("Using profile in {:?}", dir);
    fs::create_dir_all(&dir)?;

    let key = load_key(&dir.join("key"))?;

    Ok(Profile {
        key,
    })
}

fn load_key(path: &PathBuf) -> Result<String> {
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
