use env_logger::Env;
use rebuilderd_common::api::*;
use rebuilderd_common::auth::find_auth_cookie;
use rebuilderd_common::config::*;
use rebuilderd_common::errors::*;
use rebuilderd_common::Distro;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use structopt::clap::AppSettings;
use structopt::StructOpt;

pub mod auth;
pub mod config;
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
}

#[derive(Debug, StructOpt)]
struct Connect {
    pub endpoint: Option<String>,
}

fn spawn_rebuilder_script_with_heartbeat(
    client: &Client,
    distro: &Distro,
    item: &QueueItem,
) -> Result<bool> {
    let (tx, rx) = mpsc::channel();
    let t = {
        let distro = distro.clone();
        let input = item.package.url.to_string();
        thread::spawn(move || {
            let res = rebuild::rebuild(&distro, None, &input);
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

fn rebuild(client: &Client, rb: QueueItem) -> Result<()> {
    info!(
        "starting rebuild of {:?} {:?}",
        rb.package.name, rb.package.version
    );
    let distro = rb.package.distro.parse::<Distro>()?;
    let status = match spawn_rebuilder_script_with_heartbeat(&client, &distro, &rb) {
        Ok(res) => {
            if res {
                info!("Package successfully verified");
                BuildStatus::Good
            } else {
                warn!("Failed to verify package");
                BuildStatus::Bad
            }
        }
        Err(err) => {
            error!("Failed to run rebuild package: {}", err);
            BuildStatus::Fail
        }
    };
    let report = BuildReport { queue: rb, status };
    client.report_build(&report)?;
    Ok(())
}

fn run_worker_loop(client: &Client) -> Result<()> {
    loop {
        info!("requesting work");

        match client.pop_queue(&WorkQuery {}) {
            Ok(JobAssignment::Nothing) => {
                info!("no pending tasks, sleeping...");
                thread::sleep(Duration::from_secs(IDLE_DELAY));
            }
            Ok(JobAssignment::Rebuild(rb)) => rebuild(&client, rb)?,
            Err(err) => {
                error!("Failed to query for work: {}", err);
                thread::sleep(Duration::from_secs(API_ERROR_DELAY));
            }
        }

        thread::sleep(Duration::from_secs(WORKER_DELAY));
    }
}

fn run() -> Result<()> {
    let args = Args::from_args();
    let config = config::load(args.config.as_deref()).context("Failed to load config file")?;

    let cookie = find_auth_cookie().ok();
    debug!("attempt to load auth cookie resulted in: {:?}", cookie);

    if let Some(name) = args.name {
        setup::run(&name).context("Failed to setup worker")?;
    }

    match args.subcommand {
        SubCommand::Connect(connect) => {
            let system_config = rebuilderd_common::config::load(None::<String>)
                .context("Failed to load system config")?;
            let endpoint = if let Some(endpoint) = connect.endpoint {
                endpoint
            } else {
                config
                    .endpoint
                    .ok_or_else(|| format_err!("No endpoint configured"))?
            };

            let profile = auth::load()?;
            let client = profile.new_client(system_config, endpoint, config.signup_secret, cookie);
            run_worker_loop(&client)?;
        }
        SubCommand::Build(build) => {
            if rebuild::rebuild(&build.distro, build.script_location.as_ref(), &build.input)? {
                info!("Package verified successfully");
            } else {
                error!("Package failed to verify");
            }
        }
    }

    Ok(())
}

fn main() {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    if let Err(err) = run() {
        eprintln!("Error: {}", err);
        for cause in err.iter_chain().skip(1) {
            eprintln!("Because: {}", cause);
        }
        std::process::exit(1);
    }
}
