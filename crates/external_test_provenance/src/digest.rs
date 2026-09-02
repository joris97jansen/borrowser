use ring::digest::{SHA256, digest};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    pub fn parse(value: &str) -> Result<Self, DigestParseError> {
        if value.len() != 64 {
            return Err(DigestParseError::InvalidLength);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = decode_nibble(pair[0])?
                .checked_mul(16)
                .and_then(|high| decode_nibble(pair[1]).ok().map(|low| high + low))
                .ok_or(DigestParseError::InvalidHex)?;
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(64);
        for byte in self.0 {
            use std::fmt::Write;
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

pub fn sha256(bytes: &[u8]) -> Sha256Digest {
    let value = digest(&SHA256, bytes);
    let mut output = [0_u8; 32];
    output.copy_from_slice(value.as_ref());
    Sha256Digest(output)
}

fn decode_nibble(byte: u8) -> Result<u8, DigestParseError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(DigestParseError::InvalidHex),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DigestParseError {
    InvalidLength,
    InvalidHex,
}

impl fmt::Display for DigestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => {
                formatter.write_str("SHA-256 must contain 64 hexadecimal characters")
            }
            Self::InvalidHex => {
                formatter.write_str("SHA-256 must use lowercase hexadecimal characters")
            }
        }
    }
}

impl std::error::Error for DigestParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_is_lowercase_and_round_trips() {
        let digest = sha256(b"abc");
        assert_eq!(
            digest.to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(Sha256Digest::parse(&digest.to_hex()), Ok(digest));
    }

    #[test]
    fn uppercase_and_wrong_length_are_rejected() {
        assert_eq!(
            Sha256Digest::parse(&"A".repeat(64)),
            Err(DigestParseError::InvalidHex)
        );
        assert_eq!(
            Sha256Digest::parse("00"),
            Err(DigestParseError::InvalidLength)
        );
    }
}
