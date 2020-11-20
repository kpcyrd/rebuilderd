#![recursion_limit="256"]

use crate::rebuild::Context;
use env_logger::Env;
use structopt::StructOpt;
use structopt::clap::AppSettings;
use rebuilderd_common::Distro;
use rebuilderd_common::api::*;
use rebuilderd_common::auth::find_auth_cookie;
use rebuilderd_common::config::*;
use rebuilderd_common::errors::*;
use rebuilderd_common::errors::{Context as _};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::select;
use tokio::time;

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
    /// Invoke diffoscope similar to how a rebuilder would invoke it
    Diffoscope(Diffoscope),
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

#[derive(Debug, StructOpt)]
struct Diffoscope {
    pub a: String,
    pub b: String,
}

async fn spawn_rebuilder_script_with_heartbeat<'a>(client: &Client, distro: &Distro, item: &QueueItem, config: &config::ConfigFile) -> Result<Rebuild> {
    let input = item.package.url.to_string();

    let ctx = Context {
        script_location: None,
        diffoscope: config.diffoscope.clone(),
    };

    let mut rebuild = Box::pin(rebuild::rebuild(&distro, &ctx, &input));
    loop {
        select! {
            res = &mut rebuild => {
                return res;
            },
            _ = time::sleep(Duration::from_secs(PING_INTERVAL)) => {
                if let Err(err) = client.ping_build(item).await {
                    warn!("Failed to ping: {}", err);
                }
            },
        }
    }
}

async fn rebuild(client: &Client, config: &config::ConfigFile) -> Result<()> {
    info!("requesting work");
    match client.pop_queue(&WorkQuery {}).await? {
        JobAssignment::Nothing => {
            info!("no pending tasks, sleeping...");
            time::sleep(Duration::from_secs(IDLE_DELAY)).await;
        },
        JobAssignment::Rebuild(rb) => {
            info!("starting rebuild of {:?} {:?}",  rb.package.name, rb.package.version);
            let distro = rb.package.distro.parse::<Distro>()?;
            let rebuild = match spawn_rebuilder_script_with_heartbeat(&client, &distro, &rb, config).await {
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
            client.report_build(&report).await?;
        }
    }
    Ok(())
}

async fn run_worker_loop(client: &Client, config: &config::ConfigFile) -> Result<()> {
    loop {
        if let Err(err) = rebuild(client, config).await {
            error!("Failed to query for work: {:#}", err);
            time::sleep(Duration::from_secs(API_ERROR_DELAY)).await;
        }

        let restart_flag = Path::new("rebuilderd.restart");
        if restart_flag.exists() {
            info!("Restart flag exists, initiating shutdown");
            if let Err(err) = fs::remove_file(restart_flag) {
                error!("Failed to remove restart flag: {:#}", err);
            }
            return Ok(());
        }

        time::sleep(Duration::from_secs(WORKER_DELAY)).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
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
            run_worker_loop(&client, &config).await?;
        },
        SubCommand::Build(build) => {
            // this is only really for debugging
            let mut diffoscope = config::Diffoscope::default();
            if build.gen_diffoscope {
                diffoscope.enabled = true;
            }

            let res = rebuild::rebuild(&build.distro, &Context {
                script_location: build.script_location.as_ref(),
                diffoscope,
            }, &build.input).await?;

            debug!("rebuild result object is {:?}", res);

            if res.status == BuildStatus::Good {
                info!("Package verified successfully");
            } else {
                error!("Package failed to verify");
            }
        },
        SubCommand::Diffoscope(diffoscope) => {
            let output = diffoscope::diffoscope(&diffoscope.a, &diffoscope.b, &config.diffoscope).await?;
            print!("{}", output);
        },
    }

    Ok(())
}
