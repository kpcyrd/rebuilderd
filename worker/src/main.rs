#![recursion_limit="256"]

use crate::args::{Args, SubCommand};
use crate::rebuild::Context;
use env_logger::Env;
use in_toto::crypto::PrivateKey;
use structopt::StructOpt;
use rebuilderd_common::api::*;
use rebuilderd_common::auth::find_auth_cookie;
use rebuilderd_common::config::*;
use rebuilderd_common::errors::*;
use rebuilderd_common::errors::{Context as _};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;
use tokio::select;
use tokio::time;

pub mod args;
pub mod auth;
pub mod config;
pub mod diffoscope;
pub mod download;
pub mod proc;
pub mod rebuild;
pub mod setup;

async fn spawn_rebuilder_script_with_heartbeat<'a>(client: &Client, privkey: &PrivateKey, backend: config::Backend, item: &QueueItem, config: &config::ConfigFile) -> Result<Rebuild> {
    let ctx = Context {
        artifact_url: item.package.artifact_url.clone(),
        input_url: item.package.input_url.clone(),
        backend,
        build: config.build.clone(),
        diffoscope: config.diffoscope.clone(),
        privkey,
    };

    let mut rebuild = Box::pin(rebuild::rebuild(&ctx));
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

async fn rebuild(client: &Client, privkey: &PrivateKey, config: &config::ConfigFile) -> Result<()> {
    info!("Requesting work from rebuilderd...");
    let supported_backends = config.backends.keys().map(String::from).collect::<Vec<_>>();
    match client.pop_queue(&WorkQuery {
        supported_backends,
    }).await? {
        JobAssignment::Nothing => {
            info!("No pending tasks, sleeping for {}s...", IDLE_DELAY);
            time::sleep(Duration::from_secs(IDLE_DELAY)).await;
        },
        JobAssignment::Rebuild(rb) => {
            info!("Starting rebuild of {:?} {:?}",  rb.package.name, rb.package.version);

            let backend = config.backends.get(&rb.package.distro)
                .cloned()
                .ok_or_else(|| anyhow!("No backend for {:?} configured", rb.package.distro))?;

            let rebuild = match spawn_rebuilder_script_with_heartbeat(client, privkey, backend, &rb, config).await {
                Ok(res) => {
                    if res.status == BuildStatus::Good {
                        info!("Package successfully verified");
                    } else {
                        warn!("Failed to verify package");
                    };
                    res
                },
                Err(err) => {
                    error!("Unexpected error while rebuilding package package: {:#}", err);
                    Rebuild::new(BuildStatus::Fail, String::new())
                },
            };
            let report = BuildReport {
                queue: *rb,
                rebuild,
            };
            info!("Sending build report to rebuilderd...");
            client.report_build(&report)
                .await
                .context("Failed to POST to rebuilderd")?;
        }
    }
    Ok(())
}

async fn run_worker_loop(client: &Client, privkey: &PrivateKey, config: &config::ConfigFile) -> Result<()> {
    loop {
        if let Err(err) = rebuild(client, privkey, config).await {
            error!("Unexpected error, sleeping for {}s: {:#}", API_ERROR_DELAY, err);
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
    let args = Args::from_args();

    let logging = match args.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    env_logger::init_from_env(Env::default()
        .default_filter_or(logging));

    let config = config::load(&args)
        .context("Failed to load config file")?;

    let cookie = find_auth_cookie().ok();
    if cookie.is_some() {
        debug!("Successfully loaded auth cookie");
    }

    if let Some(name) = args.name {
        setup::run(&name)
            .context("Failed to setup worker")?;
    }
    let profile = auth::load()?;

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

            let client = profile.new_client(system_config, endpoint, config.signup_secret.clone(), cookie);
            run_worker_loop(&client, &profile.privkey, &config).await?;
        },
        // this is only really for debugging
        SubCommand::Build(build) => {
            let backend = if let Some(script_location) = build.script_location {
                config::Backend {
                    path: script_location,
                }
            } else {
                config.backends.get(&build.distro)
                    .cloned()
                    .ok_or_else(|| anyhow!("No backend configured in config file"))?
            };

            let diffoscope = config::Diffoscope {
                enabled: build.gen_diffoscope,
                ..Default::default()
            };

            let res = rebuild::rebuild(&Context {
                artifact_url: build.artifact_url,
                input_url: build.input_url,
                backend,
                build: config::Build::default(),
                diffoscope,
                privkey: &profile.privkey,
            }).await?;

            trace!("rebuild result object is {:?}", res);

            if res.status == BuildStatus::Good {
                info!("Package verified successfully");
            } else {
                error!("Package failed to verify");
                if let Some(diffoscope) = res.diffoscope {
                    io::stdout().write_all(diffoscope.as_bytes()).ok();
                }
            }
        },
        SubCommand::Diffoscope(diffoscope) => {
            let output = diffoscope::diffoscope(&diffoscope.a, &diffoscope.b, &config.diffoscope).await?;
            print!("{}", output);
        },
        SubCommand::CheckConfig => {
            let json = serde_json::to_string_pretty(&config)?;
            println!("{}", json);
        },
    }

    Ok(())
}
