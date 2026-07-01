use sha2::{Digest, Sha256};

/// Compute SHA-256 hex digest of a string.
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex_encode(hasher.finalize().as_slice())
}

/// Compute SHA-256 hex digest of raw bytes.
pub fn sha256_hex_bytes(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    hex_encode(hasher.finalize().as_slice())
}

/// Simple hex encoding (avoids pulling in `hex` crate).
fn hex_encode(bytes: &[u8]) -> String {
    let hex_chars = b"0123456789abcdef";
    let mut buf = vec![0u8; bytes.len() * 2];
    for (i, &byte) in bytes.iter().enumerate() {
        buf[i * 2] = hex_chars[(byte >> 4) as usize];
        buf[i * 2 + 1] = hex_chars[(byte & 0x0f) as usize];
    }
    // SAFETY: construction is valid ASCII hex
    unsafe { String::from_utf8_unchecked(buf) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hex_known_value() {
        let result = sha256_hex("hello");
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_sha256_hex_empty() {
        let result = sha256_hex("");
        assert_eq!(
            result,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_sha256_hex_bytes() {
        let result = sha256_hex_bytes(b"\x00\xff\xab");
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_hex_encode_roundtrip() {
        let bytes = [0xde, 0xad, 0xbe, 0xef];
        let hex = hex_encode(&bytes);
        assert_eq!(hex, "deadbeef");
    }
}
