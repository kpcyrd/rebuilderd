#![recursion_limit="256"]

use async_trait::async_trait;
use clap::Parser;
use crate::args::{Args, SubCommand};
use crate::rebuild::Context;
use env_logger::Env;
use in_toto::crypto::PrivateKey;
use rebuilderd_common::PkgArtifact;
use rebuilderd_common::api::*;
use rebuilderd_common::auth::find_auth_cookie;
use rebuilderd_common::config::*;
use rebuilderd_common::errors::*;
use rebuilderd_common::errors::{Context as _};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;
use tokio::time;

pub mod args;
pub mod auth;
pub mod config;
pub mod diffoscope;
pub mod download;
pub mod heartbeat;
pub mod proc;
pub mod rebuild;
pub mod setup;

pub struct HttpHeartBeat<'a> {
    client: &'a Client,
    queue_id: i32,
}

#[async_trait]
impl heartbeat::HeartBeat for HttpHeartBeat<'_> {
    fn interval(&self) -> Duration {
        Duration::from_secs(PING_INTERVAL)
    }

    async fn ping(&self) -> Result<()> {
        if let Err(err) = self.client.ping_build(&PingRequest {
            queue_id: self.queue_id,
        }).await {
            warn!("Failed to ping: {}", err);
        }
        Ok(())
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
            info!("Starting rebuild of {:?} {:?}",  rb.pkgbase.name, rb.pkgbase.version);

            let backend = config.backends.get(&rb.pkgbase.distro)
                .cloned()
                .ok_or_else(|| anyhow!("No backend for {:?} configured", rb.pkgbase.distro))?;

            let ctx = Context {
                artifacts: rb.pkgbase.artifacts.clone(),
                input_url: rb.pkgbase.input_url.clone(),
                backend,
                build: config.build.clone(),
                diffoscope: config.diffoscope.clone(),
                privkey,
            };

            let hb = HttpHeartBeat {
                client,
                queue_id: rb.id,
            };

            let mut log = Vec::new();

            let rebuilds = match rebuild::rebuild_with_heartbeat(&ctx, &mut log, &hb).await {
                Ok(res) => res,
                Err(err) => {
                    error!("Unexpected error while rebuilding package package: {:#}", err);
                    let msg = format!("rebuilderd: unexpected error while rebuilding package: {:#}\n", err);
                    if !log.is_empty() {
                        log.extend(b"\n\n");
                    }
                    log.extend(msg.as_bytes());

                    let mut res = vec![];
                    for artifact in &rb.pkgbase.artifacts {
                        res.push((
                            artifact.clone(),
                            Rebuild::new(BuildStatus::Fail)
                        ));
                    }
                    res
                },
            };

            let build_log = String::from_utf8_lossy(&log).into_owned();

            let report = BuildReport {
                queue: *rb,
                build_log,
                rebuilds,
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
    let args = Args::parse();

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

            let client = profile.new_client(system_config, endpoint, config.signup_secret.clone(), cookie)?;
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

            let mut log = Vec::new();

            let res = rebuild::rebuild(&Context {
                artifacts: vec![PkgArtifact {
                    name: "anonymous".to_string(),
                    version: "0.0.0".to_string(),
                    url: build.artifact_url,
                }],
                input_url: build.input_url,
                backend,
                build: config.build,
                diffoscope,
                privkey: &profile.privkey,
            }, &mut log).await?;

            for (artifact, res) in res {
                trace!("rebuild result object for {:?} is {:?}", artifact, res);

                if res.status == BuildStatus::Good {
                    info!("Package verified successfully");
                } else {
                    error!("Package failed to verify");
                    if let Some(diffoscope) = res.diffoscope {
                        io::stdout().write_all(diffoscope.as_bytes()).ok();
                    }
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
