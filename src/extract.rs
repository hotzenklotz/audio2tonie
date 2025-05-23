use anyhow::{anyhow, Result};
use std::{ffi::OsStr, fs::File, io::Write, path::PathBuf};
use toniefile::Toniefile;

const TONIEFILE_FRAME_SIZE: usize = 4096;

pub fn extract_tonie_to_opus(
    input_file_path: &PathBuf,
    output_file_path: Option<PathBuf>,
) -> Result<()> {
    let mut tonie_file = File::open(input_file_path)?;
    let tonie_header = Toniefile::parse_header(&mut tonie_file)?;
    let audio_data = Toniefile::extract_audio(&mut tonie_file)?;

    let output_file_path = output_file_path
        .map(|path| {
            if path.is_file() {
                path
            } else {
                path.join(
                    input_file_path
                        .with_extension("ogg")
                        .file_name()
                        .expect("Input file path must have a file name"),
                )
            }
        })
        .unwrap_or_else(|| {
            std::env::current_dir()
                .expect("Failed to get current directory")
                .join(
                    input_file_path
                        .with_extension("ogg")
                        .file_name()
                        .expect("Input file path must have a file name"),
                )
        });

    return match tonie_header.track_page_nums.len() {
        1 => {
            let mut audio_file = File::create(output_file_path)?;
            audio_file.write_all(&audio_data)?;

            return Ok(());
        }
        x if x > 1 => {
            // Split Toniefile per chapter into separate audio files
            let mut page_start: usize = 0;
            let mut page_offsets = tonie_header.track_page_nums;

            // Add final page offset, i.e. end of file
            page_offsets.push((audio_data.len() / TONIEFILE_FRAME_SIZE) as u32);

            for (i, page_offset) in page_offsets.into_iter().skip(1).enumerate() {
                let enumerated_output_file_path = output_file_path.with_file_name(format!(
                    "{}_{}",
                    i,
                    output_file_path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .expect("Expected to have a file name for output path."),
                ));

                let page_end = page_offset as usize * TONIEFILE_FRAME_SIZE;

                let mut audio_file = File::create(enumerated_output_file_path)?;
                audio_file.write_all(&audio_data[page_start..page_end])?;

                page_start = page_end;
            }

            return Ok(());
        }
        _ => Err(anyhow!("Something went wrong extracting the Tonie file.")),
    };
}
