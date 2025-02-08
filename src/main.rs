#![allow(warnings)]
mod cli;
mod converter;
mod utils;
mod opus_packet;
mod opus_page;
mod tonie_header;

#[cfg(test)]
mod tests;

use anyhow::Result;
use crate::cli::{get_cli, Command};
use crate::converter::Converter;
use crate::utils::{check_tonie_file, split_to_opus_files};
use std::path::PathBuf;

fn main() -> Result<()> {
    let cli = get_cli();

    match cli.command {
        Command::Info { input } => {
            let ok = check_tonie_file(&input)?;
            std::process::exit(if ok { 0 } else { 1 });
        }
        Command::Split { input, output } => {
            split_to_opus_files(&input, output.as_deref())?;
            std::process::exit(0);
        }
        Command::Convert {
            input,
            output,
            timestamp,
            no_tonie_header,
            bitrate,
            cbr,
            ffmpeg,
            opusenc,
            append_tonie_filename,
        } => {
            let output = if append_tonie_filename {
                format!("{}_500304E0", output.display())
            } else {
                output.display().to_string()
            };

            let converter = Converter::new();
            converter.create_tonie_file(
                &PathBuf::from(output),
                &[input],
                no_tonie_header,
                timestamp,
                bitrate,
                cbr,
                &ffmpeg,
                &opusenc,
            )?;
        }
    }

    Ok(())
}