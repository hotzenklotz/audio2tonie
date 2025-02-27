#[cfg(test)]
use std::io::{BufReader, Read};

use anyhow::Result;

pub fn vec_u8_to_i16(vector: Vec<u8>) -> Result<Vec<i16>> {
    let vec_i16 = vector
        .chunks_exact(2)
        .map(|chunk| i16::from_ne_bytes([chunk[0], chunk[1]]))
        .collect();

    return Ok(vec_i16);
}

#[cfg(test)]
pub fn are_files_equal<R: Read>(file1: R, file2: R) -> Result<bool> {
    let mut reader1 = BufReader::new(file1);
    let mut reader2 = BufReader::new(file2);

    let mut buffer1 = Vec::new();
    let mut buffer2 = Vec::new();

    reader1.read_to_end(&mut buffer1)?;
    reader2.read_to_end(&mut buffer2)?;

    return Ok(buffer1 == buffer2);
}

