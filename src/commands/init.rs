use crate::constants::{CONFIG_FILE_NAME, DEFAULT_CONFIG};
use anyhow::Result;
use std::{fs, path::Path};

pub fn run() -> Result<()> {
    if Path::new(CONFIG_FILE_NAME).exists() {
        eprintln!("[Fuse] is already initialized in this project.");
        return Ok(());
    }

    fs::write(CONFIG_FILE_NAME, DEFAULT_CONFIG)?;
    println!("Created {CONFIG_FILE_NAME}");

    Ok(())
}
