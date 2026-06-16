use std::net::SocketAddr;

#[derive(Debug, clap::Parser)]
pub struct Args {
    #[arg(short, long)]
    pub verbose: bool,
    /// API endpoint (e.g. http://localhost:8484)
    #[arg(long, env = "REBUILDERD_ENDPOINT")]
    pub endpoint: String,
    #[arg(long, env = "BIND_ADDR", default_value = "127.0.0.1:8080")]
    pub bind: SocketAddr,
}
