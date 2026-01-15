use clap::{ArgAction, Parser};

#[derive(Debug, Parser)]
pub struct Args {
    pub endpoint: Option<String>,
    #[arg(short = 'b', default_value = "127.0.0.200:0")]
    pub bind_addr: String,
    #[arg(long)]
    pub cookie: Option<String>,
    /// Verbose logging
    #[arg(short, long, action(ArgAction::Count))]
    pub verbose: u8,
    /// Do not start a test daemon
    #[arg(long)]
    pub no_daemon: bool,
}
