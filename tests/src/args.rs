use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Args {
    pub endpoint: Option<String>,
    #[structopt(short="b", default_value = "127.0.0.1:8484")]
    pub bind_addr: String,
    #[structopt(long)]
    pub cookie: String,
    /// Verbose logging
    #[structopt(short, long, parse(from_occurrences))]
    pub verbose: u8,
    /// Do not start a test daemon
    #[structopt(long)]
    pub no_daemon: bool,
}
