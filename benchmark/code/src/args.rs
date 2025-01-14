use clap::Parser;

#[derive(Parser, Debug)]
pub(crate) struct Args {
    #[arg(short, long)]
    pub dir: String,

    #[arg(short, long)]
    pub config: String,
}

impl Args {
    pub(crate) fn new() -> Args {
        Args::parse()
    }
}
