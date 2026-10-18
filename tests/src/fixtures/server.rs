use actix_web::dev::{Server, ServerHandle};
use in_toto::crypto::{PrivateKey, PublicKey};
use rebuilderd::config::Config;
use rebuilderd::db::Pool;
use rebuilderd::stats_config::StatsConfigFile;
use rebuilderd_common::api::Client;
use rebuilderd_common::errors::bail;
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use tokio_util::task::AbortOnDropHandle;

pub struct ServerHolder {
    server: Option<Server>,
    server_handle: Option<ServerHandle>,
    join: Option<AbortOnDropHandle<io::Result<()>>>,
    pub address: SocketAddr,
}

impl ServerHolder {
    pub fn new(
        pool: Pool,
        config: Config,
        private_key: PrivateKey,
    ) -> rebuilderd_common::errors::Result<Self> {
        let (server, address) =
            rebuilderd::build_server(pool, config, private_key, StatsConfigFile::default())?;

        Ok(Self {
            server: Some(server),
            server_handle: None,
            join: None,
            address,
        })
    }

    pub fn start(&mut self) -> rebuilderd_common::errors::Result<()> {
        if let Some(server) = self.server.take() {
            let handle = server.handle();
            self.server_handle = Some(handle);

            self.join = Some(AbortOnDropHandle::new(tokio::spawn(server)));

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

    pub async fn shutdown(mut self) {
        if let Some(server_handle) = self.server_handle.take() {
            server_handle.stop(false).await;
        }
        if let Some(join) = self.join.take() {
            join.await.unwrap().unwrap();
        }
    }
}

impl Drop for ServerHolder {
    fn drop(&mut self) {
        if self.server_handle.is_some() {
            panic!("IsolatedServer::shutdown wasn't called");
        }
    }
}

pub struct IsolatedServer {
    server: Option<ServerHolder>,
    pub _tmp_dir: Option<TempDir>,
    pub public_key: PublicKey,
    pub client: Client,
}

impl IsolatedServer {
    pub fn new(
        server: Option<ServerHolder>,
        tmp_dir: Option<TempDir>,
        public_key: PublicKey,
        client: Client,
    ) -> Self {
        Self {
            server,
            _tmp_dir: tmp_dir,
            public_key,
            client,
        }
    }

    pub async fn shutdown(&mut self) {
        if let Some(server) = self.server.take() {
            server.shutdown().await;
        }
    }
}
