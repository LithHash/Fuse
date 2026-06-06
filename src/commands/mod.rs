mod compile;
mod decompile;
mod init;
mod merge;

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::{config::Config, constants::HELP_TEXT};

pub fn print_help() {
    print!("{HELP_TEXT}");
}

pub fn run(args: &[String]) -> Result<()> {
    if args.len() <= 1 {
        print_help();
        return Ok(());
    }

    if args[1] == "-h" || args[1] == "--help" {
        print_help();
        return Ok(());
    }

    let command = args[1].as_str();
    let mut input: Option<PathBuf> = None;
    let mut input_b: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;

    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--input" | "-i" => {
                index += 1;
                input = Some(read_path_arg(args, index, "--input")?);
            }
            "--input-b" | "--second-input" => {
                index += 1;
                input_b = Some(read_path_arg(args, index, "--input-b")?);
            }
            "--output" | "-o" => {
                index += 1;
                output = Some(read_path_arg(args, index, "--output")?);
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            unknown => bail!("Unknown argument: {unknown}"),
        }

        index += 1;
    }

    match command {
        "init" => init::run(),
        "compile" => {
            let Some(input) = input else {
                bail!("Missing --input");
            };

            let Some(output) = output else {
                bail!("Missing --output");
            };

            let config = Config::load()?;
            compile::run(&config, &input, &output)
        }
        "decompile" => {
            let Some(input) = input else {
                bail!("Missing --input");
            };

            let Some(output) = output else {
                bail!("Missing --output");
            };

            let config = Config::load()?;
            decompile::run(&config, &input, &output)
        }
        "merge" => {
            let Some(input_a) = input else {
                bail!("Missing --input");
            };

            let Some(input_b) = input_b else {
                bail!("Missing --input-b");
            };

            let Some(output) = output else {
                bail!("Missing --output");
            };

            merge::run(&input_a, &input_b, &output)
        }
        _ => {
            print_help();
            return Ok(());
        }
    }
}

fn read_path_arg(args: &[String], index: usize, flag: &str) -> Result<PathBuf> {
    let Some(value) = args.get(index) else {
        bail!("Missing path after {flag}");
    };

    Ok(PathBuf::from(value))
}
