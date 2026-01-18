use actix_web::dev::{Server, ServerHandle};
use in_toto::crypto::{PrivateKey, PublicKey};
use rebuilderd::config::Config;
use rebuilderd::db::Pool;
use rebuilderd_common::api::Client;
use rebuilderd_common::errors::bail;
use std::net::{SocketAddr, TcpStream};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

pub struct ServerHolder {
    server: Option<Server>,
    server_handle: Mutex<Option<ServerHandle>>,
    pub address: SocketAddr,
}

impl ServerHolder {
    pub fn new(
        pool: Pool,
        config: Config,
        private_key: PrivateKey,
    ) -> rebuilderd_common::errors::Result<Self> {
        let (server, address) = rebuilderd::build_server(pool, config, private_key)?;

        Ok(Self {
            server: Some(server),
            server_handle: Mutex::default(),
            address,
        })
    }

    pub fn start(&mut self) -> rebuilderd_common::errors::Result<()> {
        if let Some(server) = self.server.take() {
            let handle = server.handle();
            self.server_handle = Mutex::new(Some(handle));

            tokio::spawn(server);

            for _ in 0..100 {
                if TcpStream::connect(self.address).is_ok() {
                    return Ok(());
                }

                thread::sleep(Duration::from_millis(100));
            }

            bail!("Failed to wait for daemon to start");
        } else {
            bail!("can't start the server more than once")
        }
    }
}

impl Drop for ServerHolder {
    fn drop(&mut self) {
        if let Some(server_handle) = self.server_handle.lock().unwrap().as_ref() {
            #[allow(clippy::let_underscore_future)]
            let _ = server_handle.stop(true);
        }
    }
}

pub struct IsolatedServer {
    _server: Option<ServerHolder>,
    pub database: Option<Pool>,
    pub public_key: PublicKey,
    pub client: Client,
}

impl IsolatedServer {
    pub fn new(
        server: Option<ServerHolder>,
        database: Option<Pool>,
        public_key: PublicKey,
        client: Client,
    ) -> Self {
        Self {
            _server: server,
            database,
            public_key,
            client,
        }
    }
}
