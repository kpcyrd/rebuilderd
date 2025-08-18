mod args;

use crate::args::Args;
use clap::Parser;
use env_logger::Env;
use rebuilderd::attestation;
use rebuilderd::config;
use rebuilderd::db;
use rebuilderd_common::errors::*;
use std::fs;

#[actix_web::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let logging = match args.verbose {
        0 => "actix_web=debug,info",
        1 => "actix_web=debug,rebuilderd=debug,rebuilderd_common=debug,info",
        2 => "debug",
        3 => "rebuilderd=trace,rebuilderd_common=trace,debug",
        _ => "trace",
    };

    env_logger::init_from_env(Env::default().default_filter_or(logging));

    dotenvy::dotenv().ok();
    let config = config::load(args.config.as_deref())?;
    if args.check_config {
        println!("{:#?}", config);
    } else if args.keygen {
        let (privkey, pubkey) = attestation::keygen_pem()?;

        println!("{}", privkey.trim_end());
        println!("{}", pubkey.trim_end());
    } else if let Some(path) = args.derive_pubkey {
        let privkey =
            fs::read(&path).with_context(|| anyhow!("Failed to read from file: {path:?}"))?;

        for privkey in attestation::pem_to_privkeys(&privkey)? {
            let privkey = privkey?;
            let pubkey = attestation::pubkey_to_pem(privkey.public())?;

            println!("{}", pubkey.trim_end());
        }
    } else {
        let privkey = attestation::load_or_create_privkey_pem(&args.signing_key)?;
        let pool = db::setup_pool("rebuilderd.db")?;
        rebuilderd::run_config(pool, config, privkey).await?;
    }
    Ok(())
}
