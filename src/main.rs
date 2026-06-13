mod commands;
mod config;
mod constants;
mod project;
mod routes;
mod watcher;

use anyhow::Result;

fn main() {
    if let Err(err) = run() {
        eprintln!("[Fuse] {err:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    commands::run(&args)
}
