use env_logger::Env;
use structopt::StructOpt;
use structopt::clap::AppSettings;
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use std::thread;
use std::time::Duration;
use rebuilderd_common::Distro;
use std::process::Command;
use std::sync::mpsc;
use rebuilderd_common::config::*;
use std::path::{Path, PathBuf};

pub mod auth;
pub mod config;
pub mod setup;

#[derive(Debug, StructOpt)]
#[structopt(global_settings = &[AppSettings::ColoredHelp])]
struct Args {
    #[structopt(short="H")]
    pub home_dir: Option<PathBuf>,
    #[structopt(subcommand)]
    pub subcommand: SubCommand,
    #[structopt(short, long)]
    pub name: Option<String>,
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
    pub distro: Distro,
    pub inputs: String,
}

#[derive(Debug, StructOpt)]
struct Connect {
    pub endpoint: Option<String>,
}

fn rebuild(distro: &Distro, input: &str) -> Result<bool> {
    // TODO: establish a common interface to interface with distro rebuilders
    let bin = match distro {
        Distro::Archlinux => "rebuilder-archlinux.sh",
        Distro::Debian => "rebuilder-debian.sh",
    };

    for prefix in &[".", "/usr/libexec/rebuilderd", "/usr/local/libexec/rebuilderd"] {
        let bin = format!("{}/{}", prefix, bin);
        let bin = Path::new(&bin);

        if bin.exists() {
            let status = Command::new(&bin)
                .args(&[input])
                .status()?;

            info!("rebuilder script finished: {:?} (for {:?})", status, input);
            return Ok(status.success());
        }
    }

    bail!("failed to find a rebuilder script")
}

fn heartbeat_rebuild(client: &Client, distro: &Distro, item: &QueueItem) -> Result<bool> {
    let (tx, rx) = mpsc::channel();
    let t = {
        let distro = distro.clone();
        let input = item.package.url.to_string();
        thread::spawn(move || {
            let res = rebuild(&distro, &input);
            tx.send(res).ok();
        })
    };

    let result = loop {
        if let Ok(result) = rx.recv_timeout(Duration::from_secs(PING_INTERVAL)) {
            break result?;
        }
        if let Err(err) = client.ping_build(item) {
            warn!("Failed to ping: {}", err);
        }
    };

    t.join().expect("Failed to join thread");
    Ok(result)
}

fn run() -> Result<()> {
    let args = Args::from_args();
    let config = config::load()?;

    if let Some(name) = args.name {
        setup::run(&name)
            .context("Failed to setup worker")?;
    }

    match args.subcommand {
        SubCommand::Connect(connect) => {
            let endpoint = connect.endpoint
                .map(|e| Ok(e))
                .unwrap_or(config.endpoint.ok_or_else(|| format_err!("No endpoint configured")))?;

            let profile = auth::load()?;
            let client = profile.new_client(endpoint);
            loop {
                info!("requesting work");
                match client.pop_queue(&WorkQuery {}) {
                    Ok(JobAssignment::Nothing) => {
                        info!("no pending tasks, sleeping...");
                        thread::sleep(Duration::from_secs(IDLE_DELAY));
                    },
                    Ok(JobAssignment::Rebuild(rb)) => {
                        info!("starting rebuild of {:?} {:?}",  rb.package.name, rb.package.version);
                        let distro = rb.package.distro.parse::<Distro>()?;
                        let status = match heartbeat_rebuild(&client, &distro, &rb) {
                            Ok(res) => {
                                if res {
                                    info!("Package successfully verified");
                                    BuildStatus::Good
                                } else {
                                    warn!("Failed to verify package");
                                    BuildStatus::Bad
                                }
                            },
                            Err(err) => {
                                error!("Failed to run rebuild package: {}", err);
                                BuildStatus::Fail
                            },
                        };
                        let report = BuildReport {
                            queue: rb,
                            status,
                        };
                        client.report_build(&report)?;
                    },
                    Err(err) => {
                        error!("failed to query for work: {}", err);
                        thread::sleep(Duration::from_secs(API_ERROR_DELAY));
                    },
                }
                thread::sleep(Duration::from_secs(WORKER_DELAY));
            }
        },
        SubCommand::Build(_) => (),
    }

    Ok(())
}

fn main() {
    env_logger::init_from_env(Env::default()
        .default_filter_or("info"));

    if let Err(err) = run() {
        eprintln!("Error: {}", err);
        for cause in err.iter_chain().skip(1) {
            eprintln!("Because: {}", cause);
        }
        std::process::exit(1);
    }
}
