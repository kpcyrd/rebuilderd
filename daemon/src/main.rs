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
    #[structopt(short)]
    verbose: bool,
    /// Configuration file path
    #[structopt(short, long)]
    config: Option<PathBuf>,
}

#[actix_web::main]
async fn main() -> Result<()> {
    let args = Args::from_args();

    let logging = if args.verbose {
        "actix_web=debug,rebuilderd=debug,info"
    } else {
        "actix_web=debug,info"
    };

    env_logger::init_from_env(Env::default()
        .default_filter_or(logging));

    dotenv::dotenv().ok();
    let config = config::load(args.config.as_deref())?;
    rebuilderd::run_config(config).await
}
