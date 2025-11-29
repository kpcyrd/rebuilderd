use crate::args::*;
use crate::config::SyncConfigFile;
use crate::fancy::Fancy;
use chrono::Utc;
use clap::Parser;
use colored::*;
use env_logger::Env;
use glob::Pattern;
use nom::AsBytes;
use rebuilderd_common::api::Client;
use rebuilderd_common::api::v1::{
    ArtifactStatus, BinaryPackage, BuildRestApi, IdentityFilter, OriginFilter, PackageReport,
    PackageRestApi, Page, Priority, QueueJobRequest, QueueRestApi, WorkerRestApi,
};
use rebuilderd_common::errors::*;
use rebuilderd_common::http;
use rebuilderd_common::utils;
use serde::Serialize;
use std::borrow::Cow;
use std::io;
use std::io::prelude::*;
use tokio::io::AsyncReadExt;

pub mod args;
pub mod config;
pub mod decompress;
pub mod fancy;
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
    let mut reports = match method {
        "archlinux" => schedule::archlinux::sync(&http, &sync).await?,
        "debian" => schedule::debian::sync(&http, &sync).await?,
        "fedora" => schedule::fedora::sync(&http, &sync).await?,
        "tails" => schedule::tails::sync(&http, &sync).await?,
        unknown => bail!(
            "No integrated sync for {:?}, use --sync-method or `pkgs sync-stdin` instead",
            unknown
        ),
    };

    reports.sort_by(|a, b| a.distribution.cmp(&b.distribution));

    if sync.print_json {
        print_json(&reports)?;
    } else {
        for report in reports {
            submit_package_report(client, &report).await?;
        }
    }

    Ok(())
}

pub async fn submit_package_report(client: &Client, sync: &PackageReport) -> Result<()> {
    let mut identity_string = "".to_owned();
    if let Some(release) = &sync.release {
        identity_string.push_str(format!("/{}", release).as_str())
    }

    if let Some(component) = &sync.component {
        identity_string.push_str(format!("/{}", component).as_str())
    }

    let display_string = format!(
        "{}{} ({})",
        sync.distribution, identity_string, sync.architecture
    );

    info!(
        "Sending {} to rebuilderd ({} packages)...",
        display_string,
        sync.packages.len()
    );

    client
        .submit_package_report(sync)
        .await
        .context("Failed to send import to daemon")?;
    Ok(())
}

async fn lookup_package(client: &Client, filter: PkgsFilter) -> Result<BinaryPackage> {
    let origin_filter = OriginFilter {
        distribution: filter.distro,
        release: None, // TODO: ls.filter.release,
        component: filter.suite,
        architecture: filter.architecture,
    };

    let identity_filter = IdentityFilter {
        name: filter.name,
        version: None, // TODO: ls.filter.version
    };

    let mut results = client
        .get_binary_packages(None, Some(&origin_filter), Some(&identity_filter))
        .await
        .context("Failed to fetch package")?;

    if results.total != 1 {
        bail!("Package lookup did not return exactly one result. Please be more specific")
    }

    Ok(results.records.pop().unwrap())
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
            for worker in client.with_auth_cookie()?.get_workers(None).await?.records {
                let label = format!("{} ({})", worker.name.green(), worker.address.yellow());
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

            // TODO: remove this after we've deprecated suite=
            if let Some(suite) = profile.suite {
                warn!(
                    "Deprecated option in config: replace `suite = \"{}\"` with `components = [\"{}\"]`",
                    suite, suite
                );
                profile.components.push(suite)
            }

            // TODO: remove this after we've deprecated architecture=
            if let Some(arch) = profile.architecture {
                warn!(
                    "Deprecated option in config: replace `architecture = \"{}\"` with `architectures = [\"{}\"]`",
                    arch, arch
                );
                profile.architectures.push(arch)
            }

            sync(
                client.with_auth_cookie()?,
                PkgsSync {
                    distro: profile.distro,
                    sync_method: profile.sync_method,
                    components: profile.components,
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
        SubCommand::Pkgs(Pkgs::SyncStdin(_sync)) => {
            let mut stdin = tokio::io::stdin();
            let mut buf = Vec::new();
            stdin.read_to_end(&mut buf).await?;

            let report = serde_json::from_slice(&buf)
                .context("Failed to deserialize pkg import from stdin")?;

            submit_package_report(client.with_auth_cookie()?, &report).await?;
        }
        SubCommand::Pkgs(Pkgs::Ls(ls)) => {
            let origin_filter = OriginFilter {
                distribution: ls.filter.distro,
                release: None, // TODO: ls.filter.release,
                component: ls.filter.suite,
                architecture: ls.filter.architecture,
            };

            let identity_filter = IdentityFilter {
                name: ls.filter.name,
                version: None, // TODO: ls.filter.version
            };

            let mut page = Page {
                limit: Some(1000),
                before: None,
                after: None,
                sort: Some("name".to_string()),
                direction: None,
            };

            loop {
                let mut results = client
                    .get_binary_packages(Some(&page), Some(&origin_filter), Some(&identity_filter))
                    .await?;

                if let Some(last) = results.records.last() {
                    page.after = Some(last.id);
                } else {
                    break;
                }

                // Filter the list by status so it's applied to the json output as well
                if let Some(status) = &ls.filter.status {
                    results.records.retain(|pkg| {
                        // If our filter is "UNKWN", match packages with status == null
                        pkg.status == ls.filter.status
                            || (*status == ArtifactStatus::Unknown && pkg.status.is_none())
                    });
                }

                if ls.json {
                    print_json(&results.records)?;
                } else {
                    let mut stdout = io::stdout();
                    for package in results.records {
                        let status_str = format!(
                            "[{}]",
                            package.status.unwrap_or(ArtifactStatus::Unknown).fancy()
                        )
                        .bold();

                        let pkg_str =
                            format!("{} {}", package.name.bold(), package.version.bold(),);

                        let info = format!(
                            "{}, {}, {}, {}",
                            package.distribution,
                            package.release.unwrap_or("<none>".to_string()),
                            package.component.unwrap_or("<none>".to_string()),
                            package.architecture,
                        );

                        if writeln!(
                            stdout,
                            "{} {:-60} ({}) {:?}",
                            status_str, pkg_str, info, package.url,
                        )
                        .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }
        SubCommand::Pkgs(Pkgs::Log(args)) => {
            let package = lookup_package(&client, args.filter).await?;
            if package.build_id.is_none() {
                bail!("Package has not been built yet");
            }

            let log = client
                .get_build_log(package.build_id.unwrap())
                .await
                .context("Failed to fetch build log")?;
            pager::write(log.as_bytes())?;
        }
        SubCommand::Pkgs(Pkgs::Diffoscope(args)) => {
            let package = lookup_package(&client, args.filter).await?;
            if package.build_id.is_none() || package.artifact_id.is_none() {
                bail!("Package has not been built yet");
            }

            let diffoscope = client
                .get_build_artifact_diffoscope(
                    package.build_id.unwrap(),
                    package.artifact_id.unwrap(),
                )
                .await
                .context("Failed to fetch diffoscope")?;

            pager::write(diffoscope.as_bytes())?;
        }
        SubCommand::Pkgs(Pkgs::Attestation(args)) => {
            let package = lookup_package(&client, args.filter).await?;
            if package.build_id.is_none() || package.artifact_id.is_none() {
                bail!("Package has not been built yet");
            }

            let attestation = client
                .get_build_artifact_attestation(
                    package.build_id.unwrap(),
                    package.artifact_id.unwrap(),
                )
                .await
                .context("Failed to fetch attestation")?;

            io::stdout().write_all(attestation.as_bytes())?;
            io::stdout().write_all(b"\n")?;
        }
        SubCommand::Queue(Queue::Ls(ls)) => {
            let mut page = Page {
                limit: if ls.head { Some(25) } else { Some(1000) },
                before: None,
                after: None,
                sort: None,
                direction: None,
            };

            loop {
                let results = client.get_queued_jobs(Some(&page), None, None).await?;
                if let Some(last) = results.records.last() {
                    page.limit = Some(1000);
                    page.after = Some(last.id);

                    if ls.head {
                        break;
                    }
                } else {
                    break;
                }

                if ls.json {
                    print_json(&results.records)?;
                } else {
                    let mut stdout = io::stdout();
                    for job in results.records {
                        let started_at = if let Some(started_at) = job.started_at {
                            started_at.format("%Y-%m-%d %H:%M:%S").to_string()
                        } else {
                            String::new()
                        };
                        let pkg_str = format!("{} {}", job.name.bold(), job.version,);

                        let running = format!(
                            "{:>11}",
                            if let Some(started_at) = job.started_at {
                                let duration = (Utc::now().naive_utc() - started_at).num_seconds();
                                Cow::Owned(utils::secs_to_human(duration))
                            } else {
                                Cow::Borrowed("")
                            }
                        );

                        if writeln!(
                            stdout,
                            "{} {:-60} {} {:19} {:?} {:?} {:?} {:?}",
                            job.queued_at
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string()
                                .bright_black(),
                            pkg_str,
                            running.green(),
                            started_at,
                            job.distribution,
                            job.release,
                            job.component,
                            job.architecture,
                        )
                        .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }
        SubCommand::Queue(Queue::Push(push)) => {
            client
                .with_auth_cookie()?
                .request_rebuild(QueueJobRequest {
                    distribution: Some(push.distro),
                    release: None, // TODO: push.release
                    component: Some(push.component),
                    name: Some(push.name),
                    version: push.version,
                    architecture: push.architecture,
                    status: None, // TODO: push.status
                    priority: Some(Priority::from(push.priority)),
                })
                .await?;
        }
        SubCommand::Queue(Queue::Delete(push)) => {
            let origin_filter = OriginFilter {
                distribution: Some(push.distro),
                release: None, // TODO: ls.filter.release,
                component: Some(push.suite),
                architecture: push.architecture,
            };

            let identity_filter = IdentityFilter {
                name: Some(push.name),
                version: push.version,
            };

            client
                .with_auth_cookie()?
                .drop_queued_jobs(Some(&origin_filter), Some(&identity_filter))
                .await?;
        }
        SubCommand::Completions(completions) => args::gen_completions(&completions)?,
    }

    Ok(())
}
