use crate::args::Args;
use colored::Colorize;
use env_logger::Env;
use rebuilderd::config::Config;
use rebuilderd_common::Distro;
use rebuilderd_common::PkgRelease;
use rebuilderd_common::Status;
use rebuilderd_common::api::*;
use rebuilderd_common::config::*;
use rebuilderd_common::errors::*;
use std::thread;
use std::time::Duration;
use std::io;
use std::io::prelude::*;
use std::net::TcpStream;
use structopt::StructOpt;
use tempfile::TempDir;

mod args;

fn list_pkgs(client: &Client) -> Result<Vec<PkgRelease>> {
    client.list_pkgs(&ListPkgs {
        name: None,
        status: None,
        distro: None,
        suite: None,
        architecture: None,
    })
}

fn initial_import(client: &Client) -> Result<()> {
    let distro = Distro::Archlinux;
    let suite = "core".to_string();
    let architecture = "x86_64".to_string();
    let pkgs = vec![
        PkgRelease::new(
            "zstd".to_string(),
            "1.4.5-1".to_string(),
            Distro::Archlinux,
            "core".to_string(),
            "x86_64".to_string(),
            "https://mirrors.kernel.org/archlinux/core/os/x86_64/zstd-1.4.5-1-x86_64.pkg.tar.zst".to_string(),
        ),
    ];

    client.sync_suite(&SuiteImport {
        distro,
        suite,
        architecture,
        pkgs,
    })?;

    Ok(())
}

fn test<T, F>(client: &Client, label: &str, f: F) -> Result<T> where
    F: FnOnce(&Client) -> Result<T>
{
    let mut stdout = io::stdout();
    write!(stdout, "{:70}", label)?;
    stdout.flush()?;

    let r = f(client);
    if r.is_ok() {
        println!("{}", "OK".green());
    } else {
        println!("{}", "ERR".red());
    }

    r
}

#[actix_rt::main]
async fn spawn_server(config: Config) {
    if let Err(err) = rebuilderd::run_config(config).await {
        error!("daemon errored: {}", err);
    }
}

fn wait_for_server() -> Result<()> {
    for _ in 0..100 {
        if TcpStream::connect("127.0.0.1:8484").is_ok() {
            return Ok(())
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("Failed to wait for daemon to start");
}

fn run(args: Args) -> Result<()> {
    let mut config = ConfigFile::default();
    if let Some(cookie) = args.cookie {
        config.auth.cookie = Some(cookie.clone());
        config.endpoints.insert(args.endpoint.clone(), EndpointConfig {
            cookie,
        });
    }

    if !args.no_daemon {
        let config = rebuilderd::config::from_struct(config.clone())?;

        let tmp_dir = TempDir::new()?;
        info!("Changing cwd to {:?}", tmp_dir);
        std::env::set_current_dir(tmp_dir.path())?;

        info!("Spawning server");
        thread::spawn(|| {
            spawn_server(config);
        });
        wait_for_server()?;
    }

    info!("Setting up client for {:?}", args.endpoint);
    let mut client = Client::new(config.clone(), Some(args.endpoint));
    client.worker_key("worker1"); // TODO: this is not a proper key

    test(&client, "Testing database to be empty", |client| {
        let pkgs = list_pkgs(&client)?;
        if !pkgs.is_empty() {
            bail!("Database is not empty");
        }
        Ok(())
    })?;

    test(&client, "Testing there is nothing to do", |client| {
        let task = client.pop_queue(&WorkQuery {})?;

        if task != JobAssignment::Nothing {
            bail!("Got a job assigned");
        }

        Ok(())
    })?;

    test(&client, "Sending initial import", initial_import)?;

    test(&client, "Testing database to contain 1 pkg", |client| {
        let pkgs = list_pkgs(&client)?;
        if pkgs.len() != 1 {
            bail!("Not 1");
        }
        Ok(())
    })?;

    test(&client, "Re-sending initial import", initial_import)?;

    test(&client, "Testing database to still contain 1 pkg", |client| {
        let mut pkgs = list_pkgs(&client)?;

        let pkg = pkgs.pop()
            .ok_or_else(|| format_err!("No pkgs found"))?;

        if pkg.name != "zstd" {
            bail!("Mismatch name");
        }

        if pkg.status != Status::Unknown {
            bail!("Status not UNKWN");
        }

        if pkg.next_retry.is_some() {
            bail!("Not None: next_retry");
        }

        if pkg.built_at.is_some() {
            bail!("Not None: built_at");
        }

        if !pkgs.is_empty() {
            bail!("Got more than 1 pkg bacK");
        }

        Ok(())
    })?;

    test(&client, "Fetching task and reporting BAD rebuild", |client| {
        let task = client.pop_queue(&WorkQuery {})?;

        let queue = match task {
            JobAssignment::Rebuild(item) => item,
            _ => bail!("Expected a job assignment"),
        };
        let rebuild = Rebuild {
            diffoscope: None,
            status: BuildStatus::Bad,
        };
        let report = BuildReport {
            queue,
            rebuild,
        };
        client.report_build(&report)?;

        Ok(())
    })?;

    test(&client, "Fetching pkg status", |client| {
        let mut pkgs = list_pkgs(&client)?;

        let pkg = pkgs.pop()
            .ok_or_else(|| format_err!("No pkgs found"))?;

        if pkg.status != Status::Bad {
            bail!("Unexpected pkg status");
        }

        if pkg.built_at.is_none() {
            bail!("Unexpected none: built_at");
        }

        if pkg.next_retry.is_none() {
            bail!("Unexpected none: next_retry");
        }

        Ok(())
    })?;

    test(&client, "Requeueing BAD pkgs", |_client| {
        client.requeue_pkgs(&RequeueQuery {
            name: None,
            status: Some(Status::Bad),
            priority: 2,
            distro: None,
            suite: None,
            architecture: None,
            reset: false,
        })?;

        Ok(())
    })?;

    test(&client, "Fetching pkg status", |client| {
        let mut pkgs = list_pkgs(&client)?;

        let pkg = pkgs.pop()
            .ok_or_else(|| format_err!("No pkgs found"))?;

        if pkg.status != Status::Bad {
            bail!("Unexpected pkg status");
        }

        if pkg.built_at.is_none() {
            bail!("Unexpected none: built_at");
        }

        if pkg.next_retry.is_none() {
            bail!("Unexpected none: next_retry");
        }

        Ok(())
    })?;

    test(&client, "Fetching task and reporting GOOD rebuild", |client| {
        let task = client.pop_queue(&WorkQuery {})?;

        let queue = match task {
            JobAssignment::Rebuild(item) => item,
            _ => bail!("Expected a job assignment"),
        };
        let rebuild = Rebuild {
            diffoscope: None,
            status: BuildStatus::Good,
        };
        let report = BuildReport {
            queue,
            rebuild,
        };
        client.report_build(&report)?;

        Ok(())
    })?;

    test(&client, "Fetching pkg status", |client| {
        let mut pkgs = list_pkgs(&client)?;

        let pkg = pkgs.pop()
            .ok_or_else(|| format_err!("No pkgs found"))?;

        if pkg.status != Status::Good {
            bail!("Unexpected pkg status");
        }

        if pkg.built_at.is_none() {
            bail!("Unexpected none: built_at");
        }

        if pkg.next_retry.is_some() {
            bail!("Unexpected some: next_retry");
        }

        Ok(())
    })?;

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::from_args();

    let logging = if args.verbose {
        "rebuilderd_tests=debug"
    } else {
        "rebuilderd_tests=info"
    };

    env_logger::init_from_env(Env::default()
        .default_filter_or(logging));

    run(args)
}
