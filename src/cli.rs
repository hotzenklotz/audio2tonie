use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CLICommands,
}

#[derive(Subcommand)]
pub enum CLICommands {
    #[command(
        about = "Extract the audio content from a Tonie file and save it as new Ogg Opus file."
    )]
    Extract {
        #[arg(required=true, help="The input audio file in Tonie format.", value_parser = validate_file_path)]
        input: PathBuf,
        #[arg(help="The output directory for saving the extracted audio content in.", value_parser = validate_directory_path)]
        output: Option<PathBuf>,
    },
    #[command(
        about = "Convert a single audio file or a directory of audio files into a Toniebox compatible audio file. Input audio files can be in any audio format that can be handled and converted by ffmpeg."
    )]
    Convert {
        #[arg(required=true, help="The input audio file or a directory of files.", value_parser = validate_directory_path)]
        input: PathBuf,
        #[arg(default_value = "500304E0", help = "The output audio file.")]
        output: PathBuf,
        #[arg(
            long,
            default_value = "ffmpeg",
            help = "Path to ffmpeg executable on your system."
        )]
        ffmpeg: String,
    },
}

fn validate_file_path(s: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(s);
    if path.exists() && path.is_file() {
        Ok(path)
    } else {
        Err(format!("The file '{}' does not exist or is not a file.", s))
    }
}

fn validate_directory_path(s: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(s);
    if path.exists() {
        Ok(path)
    } else {
        Err(format!("The path '{}' does not exist.", s))
    }
}

pub fn get_cli() -> Cli {
    Cli::parse()
}
