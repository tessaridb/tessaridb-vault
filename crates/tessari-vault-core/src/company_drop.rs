//! The statement an owner signs to ask for their company's deletion, or to cancel one: signed in the browser by the
//! owner's registered signing key, so the server cannot ask on an owner's behalf.
//!
//! Statement bytes: "tessaridb-vault/v1/company-drop" ‖ [version] ‖ [act] ‖ lp(company) ‖ u64 at, where lp is a
//! four-byte big-endian length then the bytes and `at` is seconds since 1970. The signature is stored as every
//! signature the browser makes: [version] ‖ [0x04] ‖ the 64-byte Ed25519 signature.

use crate::chain::{ChainRefused, verify_signature};

const LABEL: &[u8] = b"tessaridb-vault/v1/company-drop";
const FORMAT_VERSION: u8 = 1;

/// What the owner asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Delete the company once the waiting period ends.
    Request,
    /// Stop a deletion still waiting.
    Cancel,
}

impl Act {
    const fn byte(self) -> u8 {
        match self {
            Self::Request => 1,
            Self::Cancel => 2,
        }
    }
}

/// Why a signed drop statement does not verify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DropRefused {
    /// The signature is not a stored signature of a version this build reads.
    #[error("malformed")]
    Malformed,
    /// The signature does not verify under the owner's key.
    #[error("bad signature")]
    BadSignature,
}

/// The bytes an owner signs to `act` on `company` at `at`.
#[must_use]
pub fn statement(act: Act, company: &str, at: u64) -> Vec<u8> {
    let length = u32::try_from(company.len()).unwrap_or(u32::MAX);
    let mut bytes =
        Vec::with_capacity(LABEL.len().saturating_add(company.len()).saturating_add(14));
    bytes.extend_from_slice(LABEL);
    bytes.extend_from_slice(&[FORMAT_VERSION, act.byte()]);
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(company.as_bytes());
    bytes.extend_from_slice(&at.to_be_bytes());
    bytes
}

/// Whether `signature` is `signing_public`'s over `bytes`.
///
/// # Errors
///
/// [`DropRefused`] naming why not.
pub fn verify(signing_public: &[u8], bytes: &[u8], signature: &[u8]) -> Result<(), DropRefused> {
    verify_signature(signing_public, bytes, signature).map_err(|refused| match refused {
        ChainRefused::BadSignature => DropRefused::BadSignature,
        _ => DropRefused::Malformed,
    })
}

#[cfg(test)]
#[path = "company_drop_tests.rs"]
mod tests;
