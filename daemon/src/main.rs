use env_logger::Env;
use std::path::PathBuf;
use structopt::StructOpt;
use structopt::clap::AppSettings;
use rebuilderd::config;
use rebuilderd_common::errors::*;

#[derive(Debug, StructOpt)]
#[structopt(global_settings = &[AppSettings::ColoredHelp])]
struct Args {
    /// Verbose logging
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,
    /// Load and print a config
    #[structopt(long)]
    check_config: bool,
    /// Configuration file path
    #[structopt(short, long)]
    config: Option<PathBuf>,
}

#[actix_web::main]
async fn main() -> Result<()> {
    let args = Args::from_args();

    let logging = match args.verbose {
        0 => "actix_web=debug,info",
        1 => "actix_web=debug,rebuilderd=debug,rebuilderd_common=debug,info",
        2 => "debug",
        _ => "debug,rebuilderd=trace,rebuilderd_common=trace",
    };

    env_logger::init_from_env(Env::default()
        .default_filter_or(logging));

    dotenv::dotenv().ok();
    let config = config::load(args.config.as_deref())?;
    rebuilderd::run_config(config).await
}
