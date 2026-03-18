use clap::Parser;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, num_args(1..), help = "addresses on which to listen")]
    pub addresses: Option<Vec<String>>,

    #[arg(short, long, help = "changes the cwd immediately")]
    pub cwd: Option<String>,

    #[arg(short, long, help = "path to the settings toml")]
    pub settings: Option<String>,

    #[arg(long = "settings-name", help = "changes the name of the toml it looks for")]
    pub settings_name: Option<String>,

    #[arg(long, short = 'H', help = "sets the handler that will serve content")]
    pub handler: Option<String>,

    #[arg(long, short, help = "sets the loglevel, overrides settings")]
    pub loglevel: Option<u64>,
}
