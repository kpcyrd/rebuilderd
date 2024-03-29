use structopt::StructOpt;
use structopt::clap::AppSettings;
use std::path::PathBuf;

#[derive(Debug, StructOpt)]
#[structopt(global_settings = &[AppSettings::ColoredHelp])]
pub struct Args {
    /// Verbose logging
    #[structopt(short, long, global = true, parse(from_occurrences))]
    pub verbose: u8,
    #[structopt(subcommand)]
    pub subcommand: SubCommand,
    #[structopt(short, long)]
    pub name: Option<String>,
    #[structopt(short, long, global = true, env = "REBUILDERD_WORKER_CONFIG")]
    pub config: Option<PathBuf>,
    #[structopt(long="backend", global = true, env = "REBUILDERD_WORKER_BACKEND")]
    pub backends: Vec<String>,
}

#[derive(Debug, StructOpt)]
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

#[derive(Debug, StructOpt)]
pub struct Build {
    /// Selects the right build profile from the configuration
    pub distro: String,
    /// The pre-built artifact that should be reproduced
    pub artifact_url: String,
    /// Pass a different input file to the rebuilder backend
    #[structopt(long)]
    pub input_url: Option<String>,
    /// Use a specific rebuilder script instead of the default
    #[structopt(long)]
    pub script_location: Option<PathBuf>,
    /// Use diffoscope to generate a diff
    #[structopt(long)]
    pub gen_diffoscope: bool,
}

#[derive(Debug, StructOpt)]
pub struct Connect {
    pub endpoint: Option<String>,
}

#[derive(Debug, StructOpt)]
pub struct Diffoscope {
    pub a: PathBuf,
    pub b: PathBuf,
}
