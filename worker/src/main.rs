use env_logger::Env;
use structopt::StructOpt;
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use std::thread;
use std::time::Duration;
use rebuilderd_common::Distro;
use std::process::Command;
use std::sync::mpsc;

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
    pub distro: Distro,
    pub inputs: String,
}

#[derive(Debug, StructOpt)]
struct Connect {
    pub key: String,
    pub endpoint: String,
}

fn rebuild(distro: &Distro, input: &str) -> Result<bool> {
    // TODO: establish a common interface to interface with distro rebuilders
    let bin = match distro {
        Distro::Archlinux => "./rebuilder-archlinux.sh",
        Distro::Debian => "./rebuilder-debian.sh",
    };

    let status = Command::new(bin)
        .args(&[input])
        .status()?;

    Ok(status.success())
}

async fn heartbeat_rebuild(client: &Client, distro: &Distro, input: &str) -> Result<bool> {
    let (tx, rx) = mpsc::channel();
    let t = {
        let distro = distro.clone();
        let input = input.to_string();
        thread::spawn(move || {
            let res = rebuild(&distro, &input);
            tx.send(res).ok();
        })
    };

    let result = loop {
        if let Ok(result) = rx.recv_timeout(Duration::from_secs(60)) {
            break result?;
        }
        // TODO: this should be some kind of ticket for authentication
        let ticket = BuildTicket {
        };
        if let Err(err) = client.ping_build(&ticket).await {
            warn!("Failed to ping: {}", err);
        }
    };

    t.join().expect("Failed to join thread");
    Ok(result)
}

async fn run() -> Result<()> {
    let args = Args::from_args();

    let client = Client::new();
    match args.subcommand {
        SubCommand::Connect(connect) => {
            loop {
                info!("requesting work");
                match client.pop_queue(&WorkQuery {
                    key: connect.key.clone(),
                }).await {
                    Ok(JobAssignment::Nothing) => {
                        info!("no pending tasks, sleeping...");
                        thread::sleep(Duration::from_secs(60 * 10));
                    },
                    Ok(JobAssignment::Rebuild(rb)) => {
                        info!("starting rebuild of {:?} {:?}",  rb.package.name, rb.package.version);
                        let distro = rb.package.distro.parse::<Distro>()?;
                        let status = match heartbeat_rebuild(&client, &distro, &rb.package.url).await {
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
                            pkg: rb.package,
                            status,
                        };
                        client.report_build(&report).await?;
                    },
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
