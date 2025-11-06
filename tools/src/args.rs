use clap::{ArgAction, CommandFactory, Parser};
use clap_complete::Shell;
use glob::Pattern;
use rebuilderd_common::api::v1::ArtifactStatus;
use rebuilderd_common::errors::*;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version)]
pub struct Args {
    /// Verbose logging
    #[arg(short, long, global = true, action(ArgAction::Count))]
    pub verbose: u8,
    /// rebuilderd endpoint to talk to
    #[arg(short = 'H', long)]
    pub endpoint: Option<String>,
    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    /// Bypass tty detection and always use colors
    #[arg(short = 'C', long, global = true)]
    pub color: bool,
    #[command(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, Parser)]
pub enum SubCommand {
    /// Show worker status
    Status,
    /// Package related subcommands
    #[command(subcommand)]
    Pkgs(Pkgs),
    /// Queue related subcommands
    #[command(subcommand)]
    Queue(Queue),
    /// Generate shell completions
    Completions(Completions),
}

#[derive(Debug, Parser)]
pub enum Pkgs {
    /// Sync package index
    Sync(PkgsSync),
    /// List known packages
    Ls(PkgsList),
    /// Sync package index with profile
    SyncProfile(PkgsSyncProfile),
    /// Read a package sync from stdin
    SyncStdin(PkgsSyncStdin),
    /// Access the build log of the last rebuild
    Log(PkgsLog),
    /// Access the diffoscope of the last rebuild (if there is any)
    Diffoscope(PkgsDiffoscope),
    /// Access the attestation of the last rebuild (if there is any)
    Attestation(PkgsAttestation),
}

#[derive(Debug, Parser)]
pub struct PkgsSyncProfile {
    #[arg(long)]
    pub print_json: bool,
    pub profile: String,
    #[arg(long = "sync-config", default_value = "/etc/rebuilderd-sync.conf")]
    pub config_file: String,
}

#[derive(Debug, Parser)]
pub struct PkgsSyncStdin {}

#[derive(Debug, Parser)]
pub struct PkgsSync {
    pub distro: String,

    #[arg(long = "component")]
    pub components: Vec<String>,

    pub source: String,

    #[arg(long = "architecture")]
    pub architectures: Vec<String>,

    #[arg(long)]
    pub print_json: bool,

    #[arg(long = "maintainer")]
    pub maintainers: Vec<String>,

    #[arg(long = "release")]
    pub releases: Vec<String>,

    #[arg(long = "pkg")]
    pub pkgs: Vec<Pattern>,

    #[arg(long = "exclude")]
    pub excludes: Vec<Pattern>,

    #[arg(long)]
    pub sync_method: Option<String>,
}

#[derive(Debug, Parser)]
pub struct PkgsFilter {
    /// Filter packages matching this name
    #[arg(long)]
    pub name: Option<String>,
    /// Filter packages matching this status
    #[arg(long)]
    pub status: Option<ArtifactStatus>,
    /// Filter packages matching this distro
    #[arg(long)]
    pub distro: Option<String>,
    /// Filter packages matching this suite
    #[arg(long)]
    pub suite: Option<String>,
    /// Filter packages matching this architecture
    #[arg(long)]
    pub architecture: Option<String>,
}

#[derive(Debug, Parser)]
pub struct PkgsList {
    #[command(flatten)]
    pub filter: PkgsFilter,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Parser)]
pub struct PkgsRequeue {
    #[command(flatten)]
    pub filter: PkgsFilter,
    /// Requeue with given priority
    #[arg(long, default_value = "0")]
    pub priority: i32,
    /// Reset the status back to UNKWN
    #[arg(long)]
    pub reset: bool,
}

#[derive(Debug, Parser)]
pub struct PkgsLog {
    #[command(flatten)]
    pub filter: PkgsFilter,
}

#[derive(Debug, Parser)]
pub struct PkgsDiffoscope {
    #[command(flatten)]
    pub filter: PkgsFilter,
}

#[derive(Debug, Parser)]
pub struct PkgsAttestation {
    #[command(flatten)]
    pub filter: PkgsFilter,
}

#[derive(Debug, Parser)]
pub enum Queue {
    /// List the current build queue
    Ls(QueueList),
    /// Add a new task to the queue manually
    Push(QueuePush),
    /// Drop packages from queue matching given filter
    #[command(name = "drop")]
    Delete(QueueDrop),
}

#[derive(Debug, Parser)]
pub struct QueueList {
    #[arg(long)]
    pub head: bool,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Parser)]
pub struct QueuePush {
    pub distro: String,
    pub suite: String,

    pub name: String,
    pub version: Option<String>,

    #[arg(long)]
    pub architecture: Option<String>,
    #[arg(long, default_value = "0")]
    pub priority: i32,
}

#[derive(Debug, Parser)]
pub struct QueueDrop {
    pub distro: String,
    pub suite: String,
    #[arg(long)]
    pub architecture: Option<String>,

    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Parser)]
pub struct Completions {
    pub shell: Shell,
}

pub fn gen_completions(args: &Completions) -> Result<()> {
    clap_complete::generate(
        args.shell,
        &mut Args::command(),
        "rebuildctl",
        &mut io::stdout(),
    );
    Ok(())
}
