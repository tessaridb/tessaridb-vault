//! The vouching chain, checked the way every member's browser checks it, so the server appends only a statement that
//! keeps the chain verifiable. An append the browsers would refuse breaks the chain for every member at once: they
//! refuse a chain whole, never a verified prefix.
//!
//! Statement bytes: "tessaridb-vault/v1/statement" ‖ [version] ‖ [kind] ‖ lp(company) ‖ u64 seq ‖ prev 32 ‖
//! lp(subject) ‖ lp(agreement public) ‖ lp(signing public) ‖ signer public 32, where lp is a four-byte big-endian
//! length then the bytes. A stored signature is [version] ‖ [0x04] ‖ the 64-byte Ed25519 signature.

use std::collections::{HashMap, HashSet};

use ring::digest::{SHA256, digest};
use ring::signature::{ED25519, UnparsedPublicKey};

const LABEL: &[u8] = b"tessaridb-vault/v1/statement";
const FORMAT_VERSION: u8 = 1;
const SIGNATURE_ALGORITHM: u8 = 0x04;
const KEY_BYTES: usize = 32;
const DIGEST_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;

/// Why a chain does not verify; the same reasons a browser gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ChainRefused {
    /// The bytes are not a statement or a stored signature.
    #[error("malformed")]
    Malformed,
    /// A statement or signature of a format version this build does not read.
    #[error("unknown version")]
    UnknownVersion,
    /// The signature does not verify under the signer key the statement names.
    #[error("bad signature")]
    BadSignature,
    /// Out of place, out of shape, or a step the chain's rules do not allow.
    #[error("chain broken")]
    Broken,
    /// Signed by a key the chain does not make the owner's or an admin's.
    #[error("unvouched signer")]
    UnvouchedSigner,
}

/// One statement as stored: its bytes and its signature.
#[derive(Debug, Clone, Copy)]
pub struct Signed<'a> {
    /// The statement bytes.
    pub bytes: &'a [u8],
    /// The stored signature over them.
    pub signature: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Member,
    Admin,
    Group,
    Revoke,
    Recovery,
}

struct Decoded<'a> {
    kind: Kind,
    company: &'a [u8],
    seq: u64,
    prev: &'a [u8],
    subject: &'a [u8],
    agreement: &'a [u8],
    signing: &'a [u8],
    signer: &'a [u8],
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], ChainRefused> {
        let (taken, rest) = self
            .0
            .split_at_checked(length)
            .ok_or(ChainRefused::Malformed)?;
        self.0 = rest;
        Ok(taken)
    }

    fn byte(&mut self) -> Result<u8, ChainRefused> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(ChainRefused::Malformed)
    }

    fn field(&mut self) -> Result<&'a [u8], ChainRefused> {
        let length = <[u8; 4]>::try_from(self.take(4)?).map_err(|_| ChainRefused::Malformed)?;
        let length =
            usize::try_from(u32::from_be_bytes(length)).map_err(|_| ChainRefused::Malformed)?;
        self.take(length)
    }
}

fn decode(bytes: &[u8]) -> Result<Decoded<'_>, ChainRefused> {
    let mut reader = Reader(bytes);
    if reader.take(LABEL.len())? != LABEL {
        return Err(ChainRefused::Malformed);
    }
    if reader.byte()? != FORMAT_VERSION {
        return Err(ChainRefused::UnknownVersion);
    }
    let kind = match reader.byte()? {
        1 => Kind::Member,
        2 => Kind::Admin,
        3 => Kind::Group,
        4 => Kind::Revoke,
        5 => Kind::Recovery,
        _ => return Err(ChainRefused::Broken),
    };
    let company = reader.field()?;
    let seq = <[u8; 8]>::try_from(reader.take(8)?).map_err(|_| ChainRefused::Malformed)?;
    let decoded = Decoded {
        kind,
        company,
        seq: u64::from_be_bytes(seq),
        prev: reader.take(DIGEST_BYTES)?,
        subject: reader.field()?,
        agreement: reader.field()?,
        signing: reader.field()?,
        signer: reader.take(KEY_BYTES)?,
    };
    // Text fields must be UTF-8, as the browser's fatal decoder requires.
    for text in [decoded.company, decoded.subject] {
        std::str::from_utf8(text).map_err(|_| ChainRefused::Malformed)?;
    }
    if reader.0.is_empty() {
        Ok(decoded)
    } else {
        Err(ChainRefused::Malformed)
    }
}

fn verify_signature(signer: &[u8], message: &[u8], stored: &[u8]) -> Result<(), ChainRefused> {
    let mut reader = Reader(stored);
    if reader.byte()? != FORMAT_VERSION {
        return Err(ChainRefused::UnknownVersion);
    }
    if reader.byte()? != SIGNATURE_ALGORITHM {
        return Err(ChainRefused::Malformed);
    }
    let signature = reader.take(SIGNATURE_BYTES)?;
    if !reader.0.is_empty() {
        return Err(ChainRefused::Malformed);
    }
    UnparsedPublicKey::new(&ED25519, signer)
        .verify(message, signature)
        .map_err(|_| ChainRefused::BadSignature)
}

fn has_shape(statement: &Decoded<'_>) -> bool {
    let (agreement, signing) = (statement.agreement.len(), statement.signing.len());
    match statement.kind {
        Kind::Member => agreement == KEY_BYTES && signing == KEY_BYTES,
        Kind::Group | Kind::Recovery => agreement == KEY_BYTES && signing == 0,
        Kind::Admin | Kind::Revoke => agreement == 0 && signing == 0,
    }
}

/// Verify `chain` whole for `company` from the owner key its genesis pins.
///
/// # Errors
///
/// The [`ChainRefused`] naming the first statement's failure, in chain order.
pub fn verify(
    company: &str,
    owner_signing: &[u8],
    chain: &[Signed<'_>],
) -> Result<(), ChainRefused> {
    if chain.is_empty() {
        return Err(ChainRefused::Broken);
    }
    // Each member's signing key, the admin keys not revoked since, and the groups the chain names.
    let mut members: HashMap<&[u8], &[u8]> = HashMap::new();
    let mut admin_keys: HashSet<&[u8]> = HashSet::new();
    let mut groups: HashSet<&[u8]> = HashSet::new();
    let mut expected_prev = [0_u8; DIGEST_BYTES];
    let mut owner_account: &[u8] = &[];
    for (index, statement) in chain.iter().enumerate() {
        let decoded = decode(statement.bytes)?;
        verify_signature(decoded.signer, statement.bytes, statement.signature)?;
        let position = u64::try_from(index)
            .ok()
            .and_then(|index| index.checked_add(1));
        let placed = decoded.company == company.as_bytes()
            && Some(decoded.seq) == position
            && decoded.prev == expected_prev;
        if !placed || !has_shape(&decoded) {
            return Err(ChainRefused::Broken);
        }
        let by_owner = decoded.signer == owner_signing;
        if index == 0 {
            if !by_owner || decoded.kind != Kind::Member || decoded.signing != owner_signing {
                return Err(ChainRefused::Broken);
            }
            owner_account = decoded.subject;
        } else if !by_owner && !admin_keys.contains(decoded.signer) {
            return Err(ChainRefused::UnvouchedSigner);
        }
        match decoded.kind {
            Kind::Member => {
                if let Some(previous) = members.insert(decoded.subject, decoded.signing) {
                    admin_keys.remove(previous);
                }
            }
            Kind::Admin => {
                let signing = members.get(decoded.subject).ok_or(ChainRefused::Broken)?;
                admin_keys.insert(*signing);
            }
            Kind::Group => {
                groups.insert(decoded.subject);
            }
            // Escrow to this key reaches every member's keys, so no admin may name it.
            Kind::Recovery if !by_owner => return Err(ChainRefused::UnvouchedSigner),
            Kind::Recovery => {}
            Kind::Revoke => {
                if decoded.subject == owner_account {
                    return Err(ChainRefused::Broken);
                }
                if let Some(signing) = members.remove(decoded.subject) {
                    admin_keys.remove(signing);
                } else if !groups.remove(decoded.subject) {
                    return Err(ChainRefused::Broken);
                }
            }
        }
        let mut hashed = Vec::with_capacity(
            statement
                .bytes
                .len()
                .saturating_add(statement.signature.len()),
        );
        hashed.extend_from_slice(statement.bytes);
        hashed.extend_from_slice(statement.signature);
        expected_prev.copy_from_slice(digest(&SHA256, &hashed).as_ref());
    }
    Ok(())
}

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
