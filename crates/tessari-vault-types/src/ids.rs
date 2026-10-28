//! Identifiers: 128 random bits written as 32 lowercase hexadecimal characters.
//!
//! An identifier is opaque (it carries no name and no sequence), so it reveals nothing about how many companies or
//! accounts exist (a global sequence would). Each kind is its own type, so an account id cannot be passed where a
//! group id is expected.

use std::fmt;

/// The number of hexadecimal characters in every identifier.
const ID_HEX_LEN: usize = 32;

/// Why a text is not an identifier.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("an identifier is 32 lowercase hexadecimal characters")]
pub struct IdError;

fn hex(bytes: [u8; 16]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(ID_HEX_LEN);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

fn checked(text: &str) -> Result<String, IdError> {
    let valid = text.len() == ID_HEX_LEN
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid {
        Ok(text.to_owned())
    } else {
        Err(IdError)
    }
}

macro_rules! identifier {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// The identifier these 16 random bytes spell.
            #[must_use]
            pub fn from_random(bytes: [u8; 16]) -> Self {
                Self(hex(bytes))
            }

            /// The identifier `text` spells.
            ///
            /// # Errors
            ///
            /// [`IdError`] unless `text` is 32 lowercase hexadecimal characters.
            pub fn parse(text: &str) -> Result<Self, IdError> {
                checked(text).map(Self)
            }

            /// The identifier as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier!(
    /// An account: one person who signs in.
    AccountId
);
identifier!(
    /// A company. It is also the id of the company's own group, which every member belongs to.
    CompanyId
);
identifier!(
    /// A department or a team inside a company.
    GroupId
);
identifier!(
    /// A record in a company's vault.
    RecordId
);
identifier!(
    /// A record type a company defined.
    RecordTypeId
);
identifier!(
    /// A key's 16-byte id as the browser names it: a record key's, or a member's agreement key's.
    KeyId
);

impl CompanyId {
    /// The company's own group, which holds every member and the company's role edges.
    #[must_use]
    pub fn group(&self) -> GroupId {
        GroupId(self.0.clone())
    }
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
