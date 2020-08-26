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

async fn run(args: Args) -> Result<()> {
    dotenv::dotenv().ok();
    let config = config::load(args.config.as_deref())?;
    rebuilderd::run_config(config).await
}

#[actix_rt::main]
async fn main() {
    let args = Args::from_args();

    let logging = if args.verbose {
        "actix_web=debug,rebuilderd=debug,info"
    } else {
        "actix_web=debug,info"
    };

    env_logger::init_from_env(Env::default()
        .default_filter_or(logging));

    if let Err(err) = run(args).await {
        eprintln!("Error: {}", err);
        for cause in err.iter_chain().skip(1) {
            eprintln!("Because: {}", cause);
        }
        std::process::exit(1);
    }
}
