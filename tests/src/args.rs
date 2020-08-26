use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Args {
    #[structopt(default_value = "http://127.0.0.1:8484")]
    pub endpoint: String,
    #[structopt(long)]
    pub cookie: Option<String>,
    /// Verbose logging
    #[structopt(short)]
    pub verbose: bool,
    /// Do not start a test daemon
    #[structopt(long)]
    pub no_daemon: bool,
}
