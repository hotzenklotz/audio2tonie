use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Check and display info about Tonie file
    Info {
        /// Input file
        input: PathBuf,
    },
    /// Split Tonie file into opus tracks
    Split {
        /// Input file
        input: PathBuf,
        /// Output directory
        output: Option<PathBuf>,
    },
    /// Convert files to Tonie format
    Convert {
        /// Input file or directory
        input: PathBuf,
        /// Output file
        #[arg(default_value = "500304E0")]
        output: PathBuf,
        /// Custom timestamp
        timestamp: Option<u32>,
        /// Don't write Tonie header
        #[arg(long)]
        no_tonie_header: bool,
        /// Encoding bitrate in kbps
        #[arg(long, default_value = "96")]
        bitrate: u32,
        /// Use constant bitrate
        #[arg(long)]
        cbr: bool,
        /// Path to ffmpeg
        #[arg(long, default_value = "ffmpeg")]
        ffmpeg: String,
        /// Path to opusenc
        #[arg(long, default_value = "opusenc")]
        opusenc: String,
        /// Append [500304E0] to filename
        #[arg(long)]
        append_tonie_filename: bool,
    },
}

pub fn get_cli() -> Cli {
    Cli::parse()

}
