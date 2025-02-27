use crate::args::*;
use crate::config::SyncConfigFile;
use clap::Parser;
use colored::*;
use env_logger::Env;
use glob::Pattern;
use rebuilderd_common::api::*;
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use rebuilderd_common::utils;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt::Write as _;
use std::io;
use std::io::prelude::*;
use tokio::io::AsyncReadExt;

pub mod args;
pub mod config;
pub mod decompress;
pub mod pager;
pub mod schedule;

fn patterns_from(patterns: &[String]) -> Result<Vec<Pattern>> {
    patterns
        .iter()
        .map(|p| Pattern::new(p).map_err(Error::from))
        .collect()
}

fn print_json<S: Serialize>(x: &S) -> Result<()> {
    let mut stdout = io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &x)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub async fn sync(client: &Client, sync: PkgsSync) -> Result<()> {
    let method = if let Some(method) = &sync.sync_method {
        method.as_str()
    } else {
        sync.distro.as_str()
    };

    let http = http::client()?;
    let mut pkgs = match method {
        "archlinux" => schedule::archlinux::sync(&http, &sync).await?,
        "debian" => schedule::debian::sync(&http, &sync).await?,
        "fedora" => schedule::fedora::sync(&http, &sync).await?,
        "tails" => schedule::tails::sync(&http, &sync).await?,
        unknown => bail!(
            "No integrated sync for {:?}, use --sync-method or `pkgs sync-stdin` instead",
            unknown
        ),
    };
    pkgs.sort_by(|a, b| a.name.cmp(&b.name));

    if sync.print_json {
        print_json(&pkgs)?;
    } else {
        sync_import(
            client,
            &SuiteImport {
                distro: sync.distro,
                suite: sync.suite,
                groups: pkgs,
            },
        )
        .await?;
    }

    Ok(())
}

pub async fn sync_import(client: &Client, sync: &SuiteImport) -> Result<()> {
    info!("Sending current suite to api...");
    client
        .sync_suite(sync)
        .await
        .context("Failed to send import to daemon")?;
    Ok(())
}

async fn fetch_build_id_by_filter(client: &Client, filter: PkgsFilter) -> Result<i32> {
    let pkg = client
        .match_one_pkg(&ListPkgs {
            name: filter.name,
            status: filter.status,
            distro: filter.distro,
            suite: filter.suite,
            architecture: filter.architecture,
        })
        .await
        .context("Failed to fetch package")?;

    let build_id = pkg.build_id.context("Package has not been built yet")?;

    Ok(build_id)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let logging = match args.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    env_logger::init_from_env(Env::default().default_filter_or(logging));

    if args.color {
        debug!("Bypass tty detection and always use colors");
        colored::control::set_override(true);
    }

    let config =
        rebuilderd_common::config::load(args.config).context("Failed to load config file")?;
    let mut client = Client::new(config, args.endpoint)?;

    match args.subcommand {
        SubCommand::Status => {
            let mut stdout = io::stdout();
            for worker in client.with_auth_cookie()?.list_workers().await? {
                let label = format!("{} ({})", worker.key.green(), worker.addr.yellow());
                let status = if let Some(status) = worker.status {
                    format!("{:?}", status).bold()
                } else {
                    "idle".blue()
                };
                if writeln!(stdout, "{:-40} => {}", label, status).is_err() {
                    break;
                }
            }
        }
        SubCommand::Pkgs(Pkgs::Sync(args)) => sync(client.with_auth_cookie()?, args).await?,
        SubCommand::Pkgs(Pkgs::SyncProfile(args)) => {
            let mut config = SyncConfigFile::load(&args.config_file)?;
            let mut profile = config
                .profiles
                .remove(&args.profile)
                .ok_or_else(|| format_err!("Profile not found: {:?}", args.profile))?;

            // TODO: remove this after we've deprecated architecture=
            if let Some(arch) = profile.architecture {
                warn!("Deprecated option in config: replace `architecture = \"{}\"` with `architectures = [\"{}\"]`", arch, arch);
                profile.architectures.push(arch)
            }

            sync(
                client.with_auth_cookie()?,
                PkgsSync {
                    distro: profile.distro,
                    sync_method: profile.sync_method,
                    suite: profile.suite,
                    releases: profile.releases,
                    architectures: profile.architectures,
                    source: profile.source,

                    print_json: args.print_json,
                    maintainers: profile.maintainers,
                    pkgs: patterns_from(&profile.pkgs)?,
                    excludes: patterns_from(&profile.excludes)?,
                },
            )
            .await?;
        }
        SubCommand::Pkgs(Pkgs::SyncStdin(sync)) => {
            let mut stdin = tokio::io::stdin();
            let mut buf = Vec::new();
            stdin.read_to_end(&mut buf).await?;

            let pkgs = serde_json::from_slice(&buf)
                .context("Failed to deserialize pkg import from stdin")?;

            sync_import(
                client.with_auth_cookie()?,
                &SuiteImport {
                    distro: sync.distro,
                    suite: sync.suite,
                    groups: pkgs,
                },
            )
            .await?;
        }
        SubCommand::Pkgs(Pkgs::Ls(ls)) => {
            let pkgs = client
                .list_pkgs(&ListPkgs {
                    name: ls.filter.name,
                    status: ls.filter.status,
                    distro: ls.filter.distro,
                    suite: ls.filter.suite,
                    architecture: ls.filter.architecture,
                })
                .await?;
            if ls.json {
                print_json(&pkgs)?;
            } else {
                let mut stdout = io::stdout();
                for pkg in pkgs {
                    let status_str = format!("[{}]", pkg.status.fancy()).bold();

                    let pkg_str = format!("{} {}", pkg.name.bold(), pkg.version.bold(),);

                    let mut info = format!("{}, {}, {}", pkg.distro, pkg.suite, pkg.architecture,);

                    if let Some(build_id) = pkg.build_id {
                        write!(info, ", #{}", build_id)?;
                    }

                    if writeln!(
                        stdout,
                        "{} {:-60} ({}) {:?}",
                        status_str, pkg_str, info, pkg.artifact_url,
                    )
                    .is_err()
                    {
                        break;
                    }
                }
            }
        }
        SubCommand::Pkgs(Pkgs::Requeue(args)) => {
            client
                .with_auth_cookie()?
                .requeue_pkgs(&RequeueQuery {
                    name: args.filter.name,
                    status: args.filter.status,
                    priority: args.priority,
                    distro: args.filter.distro,
                    suite: args.filter.suite,
                    architecture: args.filter.architecture,
                    reset: args.reset,
                })
                .await?;
        }
        SubCommand::Pkgs(Pkgs::Log(args)) => {
            let build_id = fetch_build_id_by_filter(&client, args.filter).await?;
            let log = client
                .fetch_log(build_id)
                .await
                .context("Failed to fetch build log")?;
            pager::write(&log)?;
        }
        SubCommand::Pkgs(Pkgs::Diffoscope(args)) => {
            let build_id = fetch_build_id_by_filter(&client, args.filter).await?;
            let diffoscope = client
                .fetch_diffoscope(build_id)
                .await
                .context("Failed to fetch diffoscope")?;
            pager::write(&diffoscope)?;
        }
        SubCommand::Pkgs(Pkgs::Attestation(args)) => {
            let build_id = fetch_build_id_by_filter(&client, args.filter).await?;
            let mut attestation = client
                .fetch_attestation(build_id)
                .await
                .context("Failed to fetch attestation")?;
            attestation.push(b'\n');
            io::stdout().write_all(&attestation)?;
        }
        SubCommand::Queue(Queue::Ls(ls)) => {
            let limit = if ls.head { Some(25) } else { None };
            let pkgs = client.list_queue(&ListQueue { limit }).await?;

            if ls.json {
                print_json(&pkgs)?;
            } else {
                let mut stdout = io::stdout();
                for q in pkgs.queue {
                    let pkg = q.pkgbase;

                    let started_at = if let Some(started_at) = q.started_at {
                        started_at.format("%Y-%m-%d %H:%M:%S").to_string()
                    } else {
                        String::new()
                    };
                    let pkg_str = format!("{} {}", pkg.name.bold(), q.version,);

                    let running = format!(
                        "{:>11}",
                        if let Some(started_at) = q.started_at {
                            let duration = (pkgs.now - started_at).num_seconds();
                            Cow::Owned(utils::secs_to_human(duration))
                        } else {
                            Cow::Borrowed("")
                        }
                    );

                    if writeln!(
                        stdout,
                        "{} {:-60} {} {:19} {:?} {:?} {:?}",
                        q.queued_at
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()
                            .bright_black(),
                        pkg_str,
                        running.green(),
                        started_at,
                        pkg.distro,
                        pkg.suite,
                        pkg.architecture,
                    )
                    .is_err()
                    {
                        break;
                    }
                }
            }
        }
        SubCommand::Queue(Queue::Push(push)) => {
            client
                .with_auth_cookie()?
                .push_queue(&PushQueue {
                    name: push.name,
                    version: push.version,
                    priority: push.priority,
                    distro: push.distro,
                    suite: push.suite,
                    architecture: push.architecture,
                })
                .await?;
        }
        SubCommand::Queue(Queue::Delete(push)) => {
            client
                .with_auth_cookie()?
                .drop_queue(&DropQueueItem {
                    name: push.name,
                    version: push.version,
                    distro: push.distro,
                    suite: push.suite,
                    architecture: push.architecture,
                })
                .await?;
        }
        SubCommand::Completions(completions) => args::gen_completions(&completions)?,
    }

    Ok(())
}
