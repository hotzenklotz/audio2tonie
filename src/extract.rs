use anyhow::{anyhow, Result};
use std::{ffi::OsStr, fs::File, io::Write, path::PathBuf};
use toniefile::Toniefile;

pub fn extract_tonie_to_opus(
    input_file_path: &PathBuf,
    output_file_path: Option<PathBuf>,
) -> Result<()> {
    let mut tonie_file = File::open(input_file_path)?;
    let tonie_header = Toniefile::parse_header(&mut tonie_file)?;
    let audio_data = Toniefile::extract_audio(&mut tonie_file)?;

    let output_file_path = output_file_path
        .map(|path| {
            if path.file_name().is_some() {
                path
            } else {
                path.with_file_name("extracted_toniefile.ogg")
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

    println!("{:?}", output_file_path);
    println!("{:?}", tonie_header.track_page_nums);

    return match tonie_header.track_page_nums.len() {
        1 => {
            let mut audio_file = File::create(output_file_path)?;
            audio_file.write_all(&audio_data)?;

            return Ok(());
        }
        x if x > 1 => {
            for (i, page_offset) in tonie_header.track_page_nums.into_iter().enumerate() {
                let enumerated_output_file_path = output_file_path
                    .with_file_name(format!(
                        "{}_{:?}",
                        i,
                        output_file_path
                            .file_stem()
                            .unwrap_or(&OsStr::new("Extracted_Tonifile")),
                    ))
                    .with_extension("ogg");

                let mut audio_file = File::create(enumerated_output_file_path)?;
                // audio_file.write_all(&audio_data)?;
            }
            return Ok(());
        }
        _ => Err(anyhow!("Something went wrong extracting the Tonie file.")),
    };
}
