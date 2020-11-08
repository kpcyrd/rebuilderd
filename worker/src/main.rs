#![recursion_limit="256"]

use crate::rebuild::Context;
use env_logger::Env;
use structopt::StructOpt;
use structopt::clap::AppSettings;
use rebuilderd_common::api::*;
use rebuilderd_common::auth::find_auth_cookie;
use rebuilderd_common::errors::*;
use rebuilderd_common::errors::{Context as _};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;
use rebuilderd_common::Distro;
use std::sync::mpsc;
use rebuilderd_common::config::*;
use std::path::PathBuf;

pub mod auth;
pub mod config;
pub mod diffoscope;
pub mod download;
pub mod rebuild;
pub mod setup;

#[derive(Debug, StructOpt)]
#[structopt(global_settings = &[AppSettings::ColoredHelp])]
struct Args {
    #[structopt(subcommand)]
    pub subcommand: SubCommand,
    #[structopt(short, long)]
    pub name: Option<String>,
    #[structopt(short, long)]
    pub config: Option<PathBuf>,
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
    pub input: String,
    /// Use a specific rebuilder script instead of the default
    #[structopt(long)]
    pub script_location: Option<PathBuf>,
    /// Use diffoscope to generate a diff
    #[structopt(long)]
    pub gen_diffoscope: bool,
}

#[derive(Debug, StructOpt)]
struct Connect {
    pub endpoint: Option<String>,
}

fn spawn_rebuilder_script_with_heartbeat(client: &Client, distro: &Distro, item: &QueueItem, config: &config::ConfigFile) -> Result<Rebuild> {
    let (tx, rx) = mpsc::channel();
    let t = {
        let distro = distro.clone();
        let input = item.package.url.to_string();

        let ctx = Context {
            script_location: None,
            gen_diffoscope: config.gen_diffoscope,
        };

        thread::spawn(move || {
            let res = rebuild::rebuild(&distro, &ctx, &input);
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

fn rebuild(client: &Client, rb: QueueItem, config: &config::ConfigFile) -> Result<()> {
    info!("starting rebuild of {:?} {:?}",  rb.package.name, rb.package.version);
    let distro = rb.package.distro.parse::<Distro>()?;
    let rebuild = match spawn_rebuilder_script_with_heartbeat(&client, &distro, &rb, config) {
        Ok(res) => {
            if res.status == BuildStatus::Good {
                info!("Package successfully verified");
            } else {
                warn!("Failed to verify package");
            };
            res
        },
        Err(err) => {
            error!("Failed to rebuild package: {:#}", err);
            Rebuild::new(BuildStatus::Fail, Vec::new())
        },
    };
    let report = BuildReport {
        queue: rb,
        rebuild,
    };
    client.report_build(&report)?;
    Ok(())
}

fn run_worker_loop(client: &Client, config: &config::ConfigFile) -> Result<()> {
    loop {
        info!("requesting work");

        match client.pop_queue(&WorkQuery {}) {
            Ok(JobAssignment::Nothing) => {
                info!("no pending tasks, sleeping...");
                thread::sleep(Duration::from_secs(IDLE_DELAY));
            },
            Ok(JobAssignment::Rebuild(rb)) => rebuild(&client, rb, config)?,
            Err(err) => {
                error!("Failed to query for work: {:#}", err);
                thread::sleep(Duration::from_secs(API_ERROR_DELAY));
            },
        }

        let restart_flag = Path::new("rebuilderd.restart");
        if restart_flag.exists() {
            info!("Restart flag exists, initiating shutdown");
            if let Err(err) = fs::remove_file(restart_flag) {
                error!("Failed to remove restart flag: {:#}", err);
            }
            return Ok(());
        }

        thread::sleep(Duration::from_secs(WORKER_DELAY));
    }
}

fn main() -> Result<()> {
    env_logger::init_from_env(Env::default()
        .default_filter_or("info"));

    let args = Args::from_args();
    let config = config::load(args.config.as_deref())
        .context("Failed to load config file")?;

    let cookie = find_auth_cookie().ok();
    debug!("attempt to load auth cookie resulted in: {:?}",cookie);

    if let Some(name) = args.name {
        setup::run(&name)
            .context("Failed to setup worker")?;
    }

    match args.subcommand {
        SubCommand::Connect(connect) => {
            let system_config = rebuilderd_common::config::load(None::<String>)
                .context("Failed to load system config")?;
            let endpoint = if let Some(endpoint) = connect.endpoint {
                endpoint
            } else {
                config.endpoint.clone()
                    .ok_or_else(|| format_err!("No endpoint configured"))?
            };

            let profile = auth::load()?;
            let client = profile.new_client(system_config, endpoint, config.signup_secret.clone(), cookie);
            run_worker_loop(&client, &config)?;
        },
        SubCommand::Build(build) => {
            let res = rebuild::rebuild(&build.distro, &Context {
                script_location: build.script_location.as_ref(),
                gen_diffoscope: build.gen_diffoscope,
            }, &build.input)?;

            debug!("rebuild result object is {:?}", res);

            if res.status == BuildStatus::Good {
                info!("Package verified successfully");
            } else {
                error!("Package failed to verify");
            }
        },
    }

    Ok(())
}
