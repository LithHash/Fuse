mod commands;
mod config;
mod constants;

use anyhow::Result;

fn main() {
    if let Err(err) = run() {
        eprintln!("[Fuse] {err:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() <= 1 {
        commands::print_help();
        return Ok(());
    }

    commands::run(&args)
}
