use clap::{Parser, ArgAction};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version)]
pub struct Args {
    /// Verbose logging
    #[arg(short, long, global = true, action(ArgAction::Count))]
    pub verbose: u8,
    #[command(subcommand)]
    pub subcommand: SubCommand,
    #[arg(short, long)]
    pub name: Option<String>,
    #[arg(short, long, global = true, env = "REBUILDERD_WORKER_CONFIG")]
    pub config: Option<PathBuf>,
    #[arg(long = "backend", global = true, env = "REBUILDERD_WORKER_BACKEND")]
    pub backends: Vec<String>,
}

#[derive(Debug, Parser)]
pub enum SubCommand {
    /// Rebuild an individual package
    Build(Build),
    /// Connect to a central rebuilderd daemon for work
    Connect(Connect),
    /// Invoke diffoscope similar to how a rebuilder would invoke it
    Diffoscope(Diffoscope),
    /// Load and print a config
    CheckConfig,
}

#[derive(Debug, Parser)]
pub struct Build {
    /// Selects the right build profile from the configuration
    pub distro: String,
    /// The pre-built artifact that should be reproduced
    pub artifact_url: String,
    /// Pass a different input file to the rebuilder backend
    #[arg(long)]
    pub input_url: Option<String>,
    /// Use a specific rebuilder script instead of the default
    #[arg(long)]
    pub script_location: Option<PathBuf>,
    /// Use diffoscope to generate a diff
    #[arg(long)]
    pub gen_diffoscope: bool,
}

#[derive(Debug, Parser)]
pub struct Connect {
    pub endpoint: Option<String>,
}

#[derive(Debug, Parser)]
pub struct Diffoscope {
    pub a: PathBuf,
    pub b: PathBuf,
}
