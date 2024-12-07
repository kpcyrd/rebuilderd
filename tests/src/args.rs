use clap::{Parser, ArgAction};

#[derive(Debug, Parser)]
pub struct Args {
    pub endpoint: Option<String>,
    #[arg(short = 'b', default_value = "127.0.0.1:8484")]
    pub bind_addr: String,
    #[arg(long)]
    pub cookie: String,
    /// Verbose logging
    #[arg(short, long, action(ArgAction::Count))]
    pub verbose: u8,
    /// Do not start a test daemon
    #[arg(long)]
    pub no_daemon: bool,
}
