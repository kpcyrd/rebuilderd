use clap::{Parser, ArgAction};
use env_logger::Env;
use std::path::PathBuf;
use rebuilderd::config;
use rebuilderd_common::errors::*;

#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// Verbose logging
    #[arg(short, long, action(ArgAction::Count))]
    verbose: u8,
    /// Load and print a config
    #[arg(long)]
    check_config: bool,
    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,
}

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

    env_logger::init_from_env(Env::default()
        .default_filter_or(logging));

    dotenv::dotenv().ok();
    let config = config::load(args.config.as_deref())?;
    if args.check_config {
        println!("{:#?}", config);
    } else {
        rebuilderd::run_config(config).await?;
    }
    Ok(())
}
