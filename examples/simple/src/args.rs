use clap::Parser;

#[derive(Parser, Debug)]
pub(crate) struct Args {
    #[arg(short, long, default_value_t = 0)]
    pub port: u16,

    #[arg(short, long, default_value = "")]
    pub node: String,
}

impl Args {
    pub(crate) fn new() -> Args {
        Args::parse()
    }
}
