//! The passphrase `exportLibrary.db` is encrypted with.
//!
//! Unlike `master.db`, whose key is wrapped per installation, this one is the
//! same on every stick — it has to be, or no other machine could read an
//! export. rekordbox carries it obfuscated rather than in the clear, and
//! undoing that is three steps: a base85 decode, an XOR, and a zlib inflate.
//!
//! The blob and the XOR key are the ones pyrekordbox documents; the derived
//! passphrase is checked against its known prefix, so a wrong constant fails
//! here rather than as an unreadable database later.

use std::io::Read;

use flate2::read::ZlibDecoder;

/// The obfuscated passphrase, as rekordbox stores it.
const BLOB: &str = "PN_1dH8$oLJY)16j_RvM6qphWw`476>;C1cWmI#se(PG`j}~xAjlufj?`#0i{;=glh(SkW)y0>n?YEiD`l%t(";

/// XOR key, applied to the decoded blob byte by byte, repeating.
const XOR_KEY: &[u8] = b"657f48f84c437cc1";

/// RFC 1924 base85, which is not the same alphabet as Ascii85.
const ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

/// Every derived passphrase starts with this. A cheap check that the constants
/// above are intact.
const EXPECTED_PREFIX: &str = "r8gd";

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("character {0:?} is not in the base85 alphabet")]
    Alphabet(char),
    #[error("the deobfuscated blob is not zlib data: {0}")]
    Inflate(String),
    #[error("the passphrase is not valid UTF-8")]
    NotText,
    #[error("the derived passphrase does not start with {EXPECTED_PREFIX:?}; a constant is wrong")]
    WrongPrefix,
}

/// Derives the passphrase.
pub fn passphrase() -> Result<String, KeyError> {
    let decoded = base85_decode(BLOB)?;
    let xored: Vec<u8> = decoded
        .iter()
        .enumerate()
        .map(|(i, byte)| byte ^ XOR_KEY.get(i % XOR_KEY.len()).copied().unwrap_or(0))
        .collect();

    let mut inflated = Vec::new();
    ZlibDecoder::new(xored.as_slice())
        .read_to_end(&mut inflated)
        .map_err(|e| KeyError::Inflate(e.to_string()))?;

    let key = String::from_utf8(inflated).map_err(|_| KeyError::NotText)?;
    if !key.starts_with(EXPECTED_PREFIX) {
        return Err(KeyError::WrongPrefix);
    }
    Ok(key)
}

/// RFC 1924 base85. Each group of five characters is four bytes; a short final
/// group carries one byte fewer than its length.
fn base85_decode(text: &str) -> Result<Vec<u8>, KeyError> {
    let mut out = Vec::with_capacity(text.len() * 4 / 5 + 4);
    let chars: Vec<char> = text.chars().collect();
    for group in chars.chunks(5) {
        let mut value: u64 = 0;
        for character in group {
            let digit = ALPHABET
                .iter()
                .position(|c| char::from(*c) == *character)
                .ok_or(KeyError::Alphabet(*character))?;
            value = value * 85 + digit as u64;
        }
        let bytes = [
            ((value >> 24) & 0xff) as u8,
            ((value >> 16) & 0xff) as u8,
            ((value >> 8) & 0xff) as u8,
            (value & 0xff) as u8,
        ];
        let take = if group.len() == 5 { 4 } else { group.len() - 1 };
        out.extend_from_slice(bytes.get(..take).unwrap_or(&[]));
    }
    Ok(out)
}
