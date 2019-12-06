use env_logger::Env;
use std::io;
use structopt::StructOpt;
use structopt::clap::AppSettings;
use rebuilderd_common::Distro;
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use colored::*;

pub mod schedule;

#[derive(Debug, StructOpt)]
#[structopt(global_settings = &[AppSettings::ColoredHelp])]
struct Args {
    #[structopt(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, StructOpt)]
enum SubCommand {
    Status,
    Queue,
    Pkgs(Pkgs),
}

#[derive(Debug, StructOpt)]
enum Pkgs {
    Sync(PkgsSync),
    Ls(PkgsList),
}

#[derive(Debug, StructOpt)]
struct PkgsSync {
    #[structopt(long="print-json")]
    pub print_json: bool,
    pub distro: Distro,
    pub suite: String,
    pub architecture: String,
    pub source: String,
}

#[derive(Debug, StructOpt)]
struct PkgsList {
    #[structopt(long)]
    pub name: Option<String>,
    #[structopt(long)]
    pub distro: Option<String>,
    #[structopt(long)]
    pub suite: Option<String>,
    #[structopt(long)]
    pub architecture: Option<String>,
}

async fn run() -> Result<()> {
    let args = Args::from_args();

    let client = Client::new();
    match args.subcommand {
        SubCommand::Status => {
            for worker in client.list_workers().await? {
                let label = format!("{:?} ({})", worker.key, worker.addr);
                println!("{:-30} => {:?}", label, worker.status);
            }
        },
        SubCommand::Queue => (),
        SubCommand::Pkgs(Pkgs::Sync(sync)) => {
            let pkgs = match sync.distro {
                Distro::Archlinux => schedule::archlinux::sync(&sync.suite, &sync.source).await?,
                Distro::Debian => schedule::debian::sync(&sync.suite, &sync.source).await?,
            };

            if sync.print_json {
                serde_json::to_writer_pretty(io::stdout(), &pkgs)?;
            } else {
                info!("Sending current suite to api...");
                client.sync_suite(&SuiteImport {
                    distro: sync.distro,
                    suite: sync.suite,
                    architecture: sync.architecture,
                    pkgs,
                }).await?;
            }
        },
        SubCommand::Pkgs(Pkgs::Ls(ls)) => {
            let pkgs = client.list_pkgs(&ListPkgs {
                name: ls.name,
                distro: ls.distro,
                suite: ls.suite,
                architecture: ls.architecture,
            }).await?;
            for pkg in pkgs {
                let status_str = format!("[{}]", pkg.status.fancy()).bold();

                let pkg_str = format!("{} {}",
                    pkg.name.bold(),
                    pkg.version.bold(),
                );

                println!("{} {:-60} ({}, {}, {}) {:?}",
                    status_str,
                    pkg_str,
                    pkg.distro,
                    pkg.suite,
                    pkg.architecture,
                    pkg.url,
                );
            }
        },
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
