use clap::Parser;

#[derive(Parser, Debug)]
pub(crate) struct Args {
    #[arg(short, long)]
    pub dir: String,

    #[arg(short, long)]
    pub config: String,

    #[arg(short, long)]
    pub start_timestamp: u64,
}

impl Args {
    pub(crate) fn new() -> Args {
        Args::parse()
    }
}
