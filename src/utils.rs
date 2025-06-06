use anyhow::Result;

pub fn vec_u8_to_i16(vector: Vec<u8>) -> Result<Vec<i16>> {
    let vec_i16 = vector
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    return Ok(vec_i16);
}
