use crate::args::*;
use crate::config::SyncConfigFile;
use env_logger::Env;
use std::borrow::Cow;
use std::io;
use structopt::StructOpt;
use rebuilderd_common::Distro;
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use rebuilderd_common::utils;
use colored::*;

pub mod args;
pub mod config;
pub mod schedule;

pub fn sync(client: &Client, sync: PkgsSync) -> Result<()> {
    let pkgs = match sync.distro {
        Distro::Archlinux => schedule::archlinux::sync(&sync)?,
        Distro::Debian => schedule::debian::sync(&sync)?,
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
        })?;
    }

    Ok(())
}

fn run() -> Result<()> {
    let args = Args::from_args();

    let mut client = Client::new("http://127.0.0.1:8080".into());
    match args.subcommand {
        SubCommand::Status => {
            for worker in client.with_auth_cookie()?.list_workers()? {
                let label = format!("{} ({})", worker.key.green(), worker.addr.yellow());
                let status = if let Some(status) = worker.status {
                    format!("{:?}", status).bold()
                } else {
                    "idle".blue()
                };
                println!("{:-40} => {}", label, status);
            }
        },
        SubCommand::Pkgs(Pkgs::Sync(args)) => sync(client.with_auth_cookie()?, args)?,
        SubCommand::Pkgs(Pkgs::SyncProfile(args)) => {
            let mut config = SyncConfigFile::load(&args.config_file)?;
            let profile = config.profiles.remove(&args.profile)
                .ok_or_else(|| format_err!("Profile not found: {:?}", args.profile))?;
            sync(client.with_auth_cookie()?, PkgsSync {
                print_json: args.print_json,
                maintainer: profile.maintainer,
                distro: profile.distro,
                suite: profile.suite,
                architecture: profile.architecture,
                source: profile.source,
            })?;
        },
        SubCommand::Pkgs(Pkgs::Ls(ls)) => {
            let pkgs = client.list_pkgs(&ListPkgs {
                name: ls.name,
                status: ls.status,
                distro: ls.distro,
                suite: ls.suite,
                architecture: ls.architecture,
            })?;
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
        SubCommand::Queue(Queue::Ls(ls)) => {
            let limit = if ls.head {
                Some(25)
            } else {
                None
            };
            let pkgs = client.list_queue(&ListQueue {
                limit,
            })?;

            for q in pkgs.queue {
                let pkg = q.package;

                let started_at = if let Some(started_at) = q.started_at {
                    started_at.format("%Y-%m-%d %H:%M:%S").to_string()
                } else {
                    String::new()
                };
                let pkg_str = format!("{} {}",
                    pkg.name.bold(),
                    pkg.version,
                );

                let running = format!("{:11}", if let Some(started_at) = q.started_at {
                    let duration = (pkgs.now - started_at).num_seconds();
                    Cow::Owned(utils::secs_to_human(duration))
                } else {
                    Cow::Borrowed("")
                });

                println!("{} {:-60} {} {:19} {:?} {:?} {:?}",
                    q.queued_at.format("%Y-%m-%d %H:%M:%S").to_string().bright_black(),
                    pkg_str,
                    running.green(),
                    started_at,
                    pkg.distro,
                    pkg.suite,
                    pkg.architecture,
                );
            }
        },
        SubCommand::Queue(Queue::Push(push)) => {
            client.with_auth_cookie()?.push_queue(&PushQueue {
                name: push.name,
                version: push.version,
                distro: push.distro,
                suite: push.suite,
                architecture: push.architecture,
            })?;
        },
        SubCommand::Queue(Queue::Delete(push)) => {
            client.with_auth_cookie()?.drop_queue(&DropQueueItem {
                name: push.name,
                version: push.version,
                distro: push.distro,
                suite: push.suite,
                architecture: push.architecture,
            })?;
        },
        SubCommand::Completions(completions) => args::gen_completions(&completions)?,
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
