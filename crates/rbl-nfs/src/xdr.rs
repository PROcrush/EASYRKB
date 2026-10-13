//! XDR (RFC 4506) reading and writing.
//!
//! Two things here are not standard XDR, and both are Pioneer's:
//!
//! - Path and file names are **UTF-16LE**, not ASCII. The length prefix still
//!   counts bytes, so a name of `n` characters declares `2n`.
//! - Everything is still padded to a four-byte boundary, so an odd-length
//!   ASCII name and its UTF-16LE twin pad differently.

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum XdrError {
    #[error("wanted {wanted} bytes at offset {at}, buffer holds {len}")]
    Short { at: usize, wanted: usize, len: usize },
    #[error("declared length {0} is larger than any message we accept")]
    Absurd(u32),
    #[error("a UTF-16 name had an odd byte length ({0})")]
    OddUtf16(usize),
}

pub type Result<T> = std::result::Result<T, XdrError>;

/// Nothing in this protocol legitimately declares a longer field. It bounds
/// allocation from a hostile or corrupt packet.
pub const MAX_FIELD: u32 = 1 << 20;

/// Rounds a length up to the four-byte boundary XDR pads to.
#[inline]
pub const fn padded(len: usize) -> usize {
    len.div_ceil(4) * 4
}

/// A cursor over an XDR-encoded message.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub const fn position(&self) -> usize {
        self.at
    }

    pub const fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(len).ok_or(XdrError::Short {
            at: self.at,
            wanted: len,
            len: self.bytes.len(),
        })?;
        let slice = self.bytes.get(self.at..end).ok_or(XdrError::Short {
            at: self.at,
            wanted: len,
            len: self.bytes.len(),
        })?;
        self.at = end;
        Ok(slice)
    }

    pub fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([
            b.first().copied().unwrap_or(0),
            b.get(1).copied().unwrap_or(0),
            b.get(2).copied().unwrap_or(0),
            b.get(3).copied().unwrap_or(0),
        ]))
    }

    /// Fixed-width opaque data. Its length is known, so it is not prefixed.
    pub fn opaque_fixed(&mut self, len: usize) -> Result<&'a [u8]> {
        let value = self.take(len)?;
        self.take(padded(len) - len)?;
        Ok(value)
    }

    /// Variable opaque data: a length, the bytes, then padding.
    pub fn opaque(&mut self) -> Result<&'a [u8]> {
        let len = self.u32()?;
        if len > MAX_FIELD {
            return Err(XdrError::Absurd(len));
        }
        self.opaque_fixed(len as usize)
    }

    /// A standard ASCII/UTF-8 XDR string.
    pub fn string(&mut self) -> Result<String> {
        Ok(String::from_utf8_lossy(self.opaque()?).into_owned())
    }

    /// A Pioneer path or file name: UTF-16LE, byte-length prefixed.
    pub fn utf16(&mut self) -> Result<String> {
        let raw = self.opaque()?;
        if raw.len() % 2 != 0 {
            return Err(XdrError::OddUtf16(raw.len()));
        }
        let units: Vec<u16> = raw
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&units))
    }

    /// Skips a value without interpreting it, for fields we do not use.
    pub fn skip_opaque(&mut self) -> Result<()> {
        self.opaque()?;
        Ok(())
    }
}

/// Builds an XDR-encoded message.
#[derive(Debug, Default, Clone)]
pub struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub const fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self { bytes: Vec::with_capacity(capacity) }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn u32(&mut self, value: u32) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_be_bytes());
        self
    }

    fn pad_to_boundary(&mut self) {
        let overhang = self.bytes.len() % 4;
        if overhang != 0 {
            self.bytes.resize(self.bytes.len() + (4 - overhang), 0);
        }
    }

    /// Fixed-width opaque data, padded but not length-prefixed.
    pub fn opaque_fixed(&mut self, bytes: &[u8]) -> &mut Self {
        self.bytes.extend_from_slice(bytes);
        self.pad_to_boundary();
        self
    }

    /// Variable opaque data: length, bytes, padding.
    pub fn opaque(&mut self, bytes: &[u8]) -> &mut Self {
        self.u32(u32::try_from(bytes.len()).unwrap_or(0));
        self.opaque_fixed(bytes)
    }

    pub fn string(&mut self, text: &str) -> &mut Self {
        self.opaque(text.as_bytes())
    }

    /// A Pioneer path or file name: UTF-16LE, byte-length prefixed.
    pub fn utf16(&mut self, text: &str) -> &mut Self {
        let mut raw = Vec::with_capacity(text.len() * 2);
        for unit in text.encode_utf16() {
            raw.extend_from_slice(&unit.to_le_bytes());
        }
        self.opaque(&raw)
    }

    /// An optional value, encoded as a discriminant then the value.
    pub fn some(&mut self) -> &mut Self {
        self.u32(1)
    }

    pub fn none(&mut self) -> &mut Self {
        self.u32(0)
    }
}
