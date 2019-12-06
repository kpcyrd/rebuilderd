use env_logger::Env;
use structopt::StructOpt;
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use std::thread;
use std::time::Duration;

#[derive(Debug, StructOpt)]
//#[structopt(global_settings = &[AppSettings::ColoredHelp])]
struct Args {
    #[structopt(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, StructOpt)]
enum SubCommand {
    /// Rebuild an individual package
    Build(Build),
    /// Connect to a central rebuilderd daemon for work
    Connect(Connect),
}

#[derive(Debug, StructOpt)]
struct Build {
    pub distro: rebuilderd_common::Distro,
    pub inputs: Vec<String>,
}

#[derive(Debug, StructOpt)]
struct Connect {
    pub key: String,
    pub endpoint: String,
}

async fn run() -> Result<()> {
    let args = Args::from_args();
    println!("Hello, world: {:?}", args);

    let client = Client::new();
    match args.subcommand {
        SubCommand::Connect(connect) => {
            loop {
                info!("requesting work");
                match client.get_work(&WorkQuery {
                    key: connect.key.clone(),
                }).await {
                    Ok(JobAssignment::Nothing) => info!("no pending tasks"),
                    Err(err) => {
                        error!("failed to query for work: {}", err);
                    },
                }
                thread::sleep(Duration::from_secs(5));
            }
        },
        SubCommand::Build(_) => (),
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(Env::default()
        .default_filter_or("info"));

    if let Err(err) = run().await {
        eprintln!("Error: {}", err);
        for cause in err.iter_chain().skip(1) {
            eprintln!("Because: {}", cause);
        }
        std::process::exit(1);
    }
}
