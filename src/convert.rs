use anyhow::{anyhow, Result};
use human_sort::compare;
use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use toniefile::Toniefile;

use crate::utils::vec_u8_to_i16;

const SUPPORTED_FILE_EXTENSIONS: [&str; 6] = ["mp3", "aac", "wav", "ogg", "webm", "opus"];

pub fn convert_to_tonie(
    input_file_path: &PathBuf,
    output_file_path: &PathBuf,
    ffmpeg: String,
) -> Result<File> {
    // Converts an input file into Tonie compatible Ogg Opus audio file with the custom Tonie header and correctly sized 4kb opus content blocks.
    // If the input is a directory then all files will be converted into a single Tonie file with multiple chapters.

    let input_files = filter_input_files(input_file_path)?;

    // Use the input file name as a Opus header metadata comment
    // Make it easier to identify already encoded files without listening to them
    let user_comments = input_files
        .first()
        .and_then(|file_path| file_path.file_name())
        .and_then(|os_str| os_str.to_str())
        .map(|file_name| vec![file_name]);

    let output_file = File::create(output_file_path)?;
    let mut toniefile = Toniefile::new(&output_file, 0x12345678, user_comments).unwrap();

    input_files
        .iter()
        .filter_map(|input_file| {
            audiofile_to_wav(input_file, &ffmpeg)
                .and_then(vec_u8_to_i16)
                .ok()
        })
        .enumerate()
        .for_each(|(index, buffer)| {
            toniefile.encode(&buffer[..]).ok();

            if input_files.len() > 1 && index < input_files.len() - 1 {
                // When providing several input files, when encode them as one audio file with separate chapters
                // Skip this if there is only one file and for the last file in a collection
                toniefile.new_chapter().ok();
            }
        });

    toniefile.finalize_no_consume()?;

    return Ok(output_file);
}

pub fn audiofile_to_wav(file_path: &PathBuf, ffmpeg: &str) -> Result<Vec<u8>> {
    let ffmpeg_process = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "warning",
            "-i",
            file_path.to_str().unwrap(),
            "-f",
            "wav",
            "-ar",
            "48000",
            "-",
        ])
        .stdout(Stdio::piped())
        .spawn()?;

    // Await processes to finish
    let ffmpeg_status = ffmpeg_process.wait_with_output()?;
    if !ffmpeg_status.status.success() {
        return Err(anyhow!(
            "Conversion with ffmpeg failed: {}",
            ffmpeg_status.status
        ));
    }

    return Ok(ffmpeg_status.stdout);
}

pub fn filter_input_files(input_file: &PathBuf) -> Result<Vec<PathBuf>> {
    if input_file.is_file() && is_file_extension_supported(&input_file) {
        return Ok(vec![input_file.to_path_buf()]);
    } else if input_file.is_dir() {
        let mut paths = std::fs::read_dir(input_file)?
            .filter_map(|res| res.ok())
            .map(|dir_entry| dir_entry.path())
            .filter(is_file_extension_supported)
            .collect::<Vec<_>>();

        paths.sort_by(|a, b| {
            compare(
                &a.file_name().expect("Unable to read file name").to_string_lossy(),
                &b.file_name().expect("Unable to read file name").to_string_lossy(),
            )
        });

        return Ok(paths);
    } else {
        return Err(anyhow!["Could not process the provided input files. Expected the input file to end in one of the follow extensions: {:?}", SUPPORTED_FILE_EXTENSIONS]);
    }
}

fn is_file_extension_supported(input_file_path: &PathBuf) -> bool {
    return input_file_path.extension().map_or(false, |ext| {
        SUPPORTED_FILE_EXTENSIONS
            .contains(&ext.to_str().expect("Could not identify file extension."))
    });
}
