mod cli;
mod convert;
mod utils;

#[cfg(test)]
mod tests;

use crate::cli::{get_cli, CLICommands};
use crate::convert::convert_to_tonie;
use anyhow::Result;

fn main() -> Result<()> {
    let cli = get_cli();

    match cli.command {
        CLICommands::Extract { input, output } => {
            std::process::exit(0);
        }
        CLICommands::Convert {
            input,
            output,
            ffmpeg,
        } => convert_to_tonie(&input, &output, ffmpeg)?,
    };

    Ok(())
}
