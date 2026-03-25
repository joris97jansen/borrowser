use crate::html5::shared::{AtomId, AtomTable, Token};

pub(super) fn token_discriminant(token: &Token) -> u64 {
    match token {
        Token::Doctype { .. } => 1,
        Token::StartTag { .. } => 2,
        Token::EndTag { .. } => 3,
        Token::Comment { .. } => 4,
        Token::Text { .. } => 5,
        Token::Eof => 6,
    }
}

pub(super) fn mix_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    if hash == 0 {
        hash = 0xcbf29ce484222325;
    }
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(super) fn mix_u64(hash: u64, value: u64) -> u64 {
    mix_bytes(hash, &value.to_le_bytes())
}

pub(super) fn mix_atom_name(hash: u64, atoms: &AtomTable, id: AtomId) -> u64 {
    if let Some(name) = atoms.resolve(id) {
        return mix_bytes(hash, name.as_bytes());
    }
    mix_u64(hash, u64::from(id.0))
}
