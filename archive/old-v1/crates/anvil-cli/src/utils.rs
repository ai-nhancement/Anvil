use std::fmt::Write as _;

use sha2::Digest as _;

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for b in &digest {
        write!(hex, "{b:02x}").unwrap();
    }
    hex
}
