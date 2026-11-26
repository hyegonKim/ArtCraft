use crate::{FormatError, Result};
use base64::Engine as _;
use std::io::{Read, Write};

pub(super) fn base64_engine() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

pub(super) fn decode_embedded(strdata: &str) -> std::result::Result<Vec<u8>, String> {
    let raw = base64_engine().decode(strdata.split_whitespace().collect::<String>()).map_err(|e| format!("bad base64: {e}"))?;
    let (length, compressed) = raw.split_at_checked(4).ok_or("too short")?;
    let length = u32::from_be_bytes([length[0], length[1], length[2], length[3]]) as usize;
    let mut out = Vec::new();

    // Reading one extra byte detects data that contradicts the declared format length
    let mut decoder = flate2::read::ZlibDecoder::new(compressed).take(length as u64 + 1);
    decoder.read_to_end(&mut out).map_err(|e| format!("bad compressed data: {e}"))?;
    if out.len() != length {
        return Err(format!("{} bytes where {length} were declared", out.len()));
    }

    Ok(out)
}

pub(super) fn encode_embedded(bytes: &[u8]) -> Result<String> {
    let length = u32::try_from(bytes.len()).map_err(|_| FormatError::Io("resource too large".into()))?;
    let mut encoder = flate2::write::ZlibEncoder::new(length.to_be_bytes().to_vec(), flate2::Compression::default());
    encoder.write_all(bytes).map_err(|e| FormatError::Io(e.to_string()))?;
    let raw = encoder.finish().map_err(|e| FormatError::Io(e.to_string()))?;
    Ok(base64_engine().encode(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompression_requires_the_declared_length() {
        let encoded = encode_embedded(&[1, 2, 3, 4]).unwrap();

        assert_eq!(decode_embedded(&encoded).unwrap(), [1, 2, 3, 4]);

        let mut raw = base64_engine().decode(&encoded).unwrap();
        for length in [1_u32, 5] {
            raw[..4].copy_from_slice(&length.to_be_bytes());

            assert!(decode_embedded(&base64_engine().encode(&raw)).unwrap_err().contains("declared"));
        }
    }
}
