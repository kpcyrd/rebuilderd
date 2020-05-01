use rebuilderd_common::{Distro, Status};
use rebuilderd_common::errors::*;
use glob::Pattern;
use std::io::stdout;
use std::path::PathBuf;
use structopt::StructOpt;
use structopt::clap::{AppSettings, Shell};

#[derive(Debug, StructOpt)]
#[structopt(global_settings = &[AppSettings::ColoredHelp])]
pub struct Args {
    /// rebuilderd endpoint to talk to
    #[structopt(short="H", long)]
    pub endpoint: Option<String>,
    /// Configuration file path
    #[structopt(short, long)]
    pub config: Option<PathBuf>,
    /// Bypass tty detection and always use colors
    #[structopt(long, global=true)]
    pub color: bool,
    /// Verbose logging
    #[structopt(short)]
    pub verbose: bool,
    #[structopt(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, StructOpt)]
pub enum SubCommand {
    Status,
    Pkgs(Pkgs),
    Queue(Queue),
    /// Generate shell completions
    Completions(Completions),
}

#[derive(Debug, StructOpt)]
pub enum Pkgs {
    Sync(PkgsSync),
    Ls(PkgsList),
    SyncProfile(PkgsSyncProfile),
    Requeue(PkgsRequeue),
}

#[derive(Debug, StructOpt)]
pub struct PkgsSyncProfile {
    #[structopt(long="print-json")]
    pub print_json: bool,
    pub profile: String,
    #[structopt(long="sync-config", default_value="/etc/rebuilderd-sync.conf")]
    pub config_file: String,
}

#[derive(Debug, StructOpt)]
pub struct PkgsSync {
    pub distro: Distro,
    pub suite: String,
    pub architecture: String,
    pub source: String,
    #[structopt(long="print-json")]
    pub print_json: bool,
    #[structopt(long="maintainer")]
    pub maintainers: Vec<String>,
    #[structopt(long="pkg")]
    pub pkgs: Vec<Pattern>,
    #[structopt(long="exclude")]
    pub excludes: Vec<Pattern>,
}

#[derive(Debug, StructOpt)]
pub struct PkgsList {
    #[structopt(long)]
    pub name: Option<String>,
    #[structopt(long, possible_values=&["GOOD", "BAD", "UNKWN"])]
    pub status: Option<Status>,
    #[structopt(long)]
    pub distro: Option<String>,
    #[structopt(long)]
    pub suite: Option<String>,
    #[structopt(long)]
    pub architecture: Option<String>,
    #[structopt(long)]
    pub json: bool,
}

#[derive(Debug, StructOpt)]
pub struct PkgsRequeue {
    #[structopt(long)]
    pub name: Option<String>,
    #[structopt(long, possible_values=&["GOOD", "BAD", "UNKWN"])]
    pub status: Option<Status>,
    #[structopt(long)]
    pub distro: Option<String>,
    #[structopt(long)]
    pub suite: Option<String>,
    #[structopt(long)]
    pub architecture: Option<String>,
    #[structopt(long)]
    pub reset: bool,
}

#[derive(Debug, StructOpt)]
pub enum Queue {
    Ls(QueueList),
    Push(QueuePush),
    #[structopt(name="drop")]
    Delete(QueueDrop),
}

#[derive(Debug, StructOpt)]
pub struct QueueList {
    #[structopt(long)]
    pub head: bool,
    #[structopt(long)]
    pub json: bool,
}

#[derive(Debug, StructOpt)]
pub struct QueuePush {
    pub distro: String,
    pub suite: String,
    #[structopt(long)]
    pub architecture: Option<String>,

    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, StructOpt)]
pub struct QueueDrop {
    pub distro: String,
    pub suite: String,
    #[structopt(long)]
    pub architecture: Option<String>,

    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, StructOpt)]
pub struct Completions {
    #[structopt(possible_values=&Shell::variants())]
    pub shell: Shell,
}

pub fn gen_completions(args: &Completions) -> Result<()> {
    Args::clap().gen_completions_to("rebuildctl", args.shell, &mut stdout());
    Ok(())
}
