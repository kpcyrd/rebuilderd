pub mod server;

use crate::args::Args;
use crate::fixtures::server::{IsolatedServer, ServerHolder};
use clap::Parser;
use in_toto::crypto::{KeyType, PrivateKey, SignatureScheme};
use rand::distr::{Alphanumeric, SampleString};
use rebuilderd::db;
use rebuilderd_common::api::Client;
use rebuilderd_common::config::{ConfigFile, EndpointConfig};
use rebuilderd_common::errors::info;
use rstest::fixture;
use tempfile::TempDir;

#[fixture]
pub fn program_arguments() -> Args {
    Args::parse()
}

#[fixture]
pub fn config_file(program_arguments: Args) -> ConfigFile {
    let mut config = ConfigFile::default();

    let cookie = program_arguments
        .cookie
        .unwrap_or(Alphanumeric.sample_string(&mut rand::rng(), 32));
    config.auth.cookie = Some(cookie.clone());

    let signup_secret = Alphanumeric.sample_string(&mut rand::rng(), 32);
    config.worker.signup_secret = Some(signup_secret);

    let addr = program_arguments.bind_addr;
    let endpoint = program_arguments
        .endpoint
        .unwrap_or_else(|| format!("http://{}", addr));

    config.http.bind_addr = Some(addr.clone());
    config.endpoints.insert(
        endpoint.clone(),
        EndpointConfig {
            cookie: cookie.clone(),
        },
    );

    config
}

#[fixture]
pub fn private_key() -> PrivateKey {
    let privkey = PrivateKey::new(KeyType::Ed25519).expect("Failed to generate private key");

    PrivateKey::from_pkcs8(&privkey, SignatureScheme::Ed25519)
        .expect("Failed to use generated private key")
}

fn make_client(config_file: ConfigFile, endpoint: String) -> Client {
    info!("Setting up client for {:?}", endpoint);
    let mut client = Client::new(config_file.clone(), Some(endpoint)).unwrap();

    // we assume these are in the config since we generate random ones unless they're provided
    client.auth_cookie(config_file.auth.cookie.unwrap());
    client.signup_secret(config_file.worker.signup_secret.unwrap());

    let worker_key = Alphanumeric.sample_string(&mut rand::rng(), 32);
    client.worker_key(worker_key);

    client
}

#[fixture]
pub fn isolated_server(
    program_arguments: Args,
    config_file: ConfigFile,
    private_key: PrivateKey,
) -> IsolatedServer {
    let public_key = private_key.public().clone();
    let config = rebuilderd::config::from_struct(
        config_file.clone(),
        config_file.auth.cookie.clone().unwrap(),
    )
    .unwrap();

    let (server, pool, endpoint) = if !program_arguments.no_daemon {
        let pool = {
            let tmp_dir = TempDir::new().unwrap();
            let database_path = tmp_dir.path().join("rebuilderd.db");

            db::setup_pool(database_path.to_str().unwrap()).unwrap()
        };

        let mut server = ServerHolder::new(pool.clone(), config, private_key).unwrap();
        server.start().unwrap();

        let endpoint = format!("http://{}", server.address.to_string());
        (Some(server), Some(pool), endpoint)
    } else {
        let addr = program_arguments.bind_addr;
        let endpoint = program_arguments
            .endpoint
            .unwrap_or_else(|| format!("http://{}", addr));

        (None, None, endpoint)
    };

    let client = make_client(config_file, endpoint);

    IsolatedServer::new(server, pool, public_key, client)
}
