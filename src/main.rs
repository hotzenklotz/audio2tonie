mod cli;
mod convert;
mod extract;
mod utils;

#[cfg(test)]
mod tests;

use crate::cli::{get_cli, CLICommands};
use crate::convert::convert_to_tonie;
use anyhow::Result;
use extract::extract_tonie_to_opus;

fn main() -> Result<()> {
    let cli = get_cli();

    match cli.command {
        CLICommands::Extract { input, output } => {
            extract_tonie_to_opus(&input, output)?;
        }
        CLICommands::Convert {
            input,
            output,
            ffmpeg,
        } => {
            convert_to_tonie(&input, &output, ffmpeg)?;
        }
    };

    Ok(())
}
