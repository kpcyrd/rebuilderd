use std::net::SocketAddr;
use url::Url;

#[derive(Debug, clap::Parser)]
pub struct Args {
    #[arg(short, long)]
    pub verbose: bool,
    /// API endpoint (e.g. http://localhost:8484)
    #[arg(long, env = "REBUILDERD_ENDPOINT")]
    pub endpoint: String,
    #[arg(long, env = "BIND_ADDR", default_value = "127.0.0.1:8080")]
    pub bind: SocketAddr,

    #[arg(long, env = "OIDC_CLIENT_ID")]
    pub oidc_client_id: String,
    #[arg(long, env = "OIDC_CLIENT_SECRET")]
    pub oidc_client_secret: String,
    #[arg(long, env = "OIDC_ISSUER")]
    pub oidc_issuer: Url,
    #[arg(long, env = "OIDC_REDIRECT_URI")]
    pub oidc_redirect_uri: Url,
}
