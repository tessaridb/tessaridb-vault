//! A `member_keys` row and what it reads as.

use tessari_vault_constants::{
    MAX_WRAPPED_KEYS_BYTES, PUBLIC_KEY_BYTES, UNLOCK_KDF_MAX_LANES, UNLOCK_KDF_MAX_MEMORY_KIB,
    UNLOCK_KDF_MAX_PASSES, UNLOCK_KDF_MIN_LANES, UNLOCK_KDF_MIN_MEMORY_KIB, UNLOCK_KDF_MIN_PASSES,
    UNLOCK_SALT_BYTES,
};
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::{AccountId, KeyId, base64url};

use crate::{Error, Result};

/// One `member_keys` row: the public half of a member's key set and the derivation that unlocks the private half.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeyEntity {
    pub(crate) account: String,
    pub(crate) key_id: String,
    pub(crate) agreement_public: String,
    pub(crate) signing_public: String,
    pub(crate) kdf_salt: String,
    pub(crate) kdf_memory: i64,
    pub(crate) kdf_passes: i64,
    pub(crate) kdf_lanes: i64,
}

impl FromRecord for KeyEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            account: row.take("account")?,
            key_id: row.take("key_id")?,
            agreement_public: row.take("agreement_public")?,
            signing_public: row.take("signing_public")?,
            kdf_salt: row.take("kdf_salt")?,
            kdf_memory: row.take("kdf_memory")?,
            kdf_passes: row.take("kdf_passes")?,
            kdf_lanes: row.take("kdf_lanes")?,
        })
    }
}

/// The parameters the browser derives the unlock key with, inside the floor and the ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kdf {
    salt: Vec<u8>,
    memory_kib: u32,
    passes: u32,
    lanes: u32,
}

impl Kdf {
    /// The parameters, refused outside the bounds the browser itself enforces.
    ///
    /// # Errors
    ///
    /// When the salt is not 16 bytes or a cost is outside its bounds.
    pub fn new(salt: Vec<u8>, memory_kib: u32, passes: u32, lanes: u32) -> Result<Self> {
        let within = salt.len() == UNLOCK_SALT_BYTES
            && (UNLOCK_KDF_MIN_MEMORY_KIB..=UNLOCK_KDF_MAX_MEMORY_KIB).contains(&memory_kib)
            && (UNLOCK_KDF_MIN_PASSES..=UNLOCK_KDF_MAX_PASSES).contains(&passes)
            && (UNLOCK_KDF_MIN_LANES..=UNLOCK_KDF_MAX_LANES).contains(&lanes);
        if within {
            Ok(Self {
                salt,
                memory_kib,
                passes,
                lanes,
            })
        } else {
            Err(Error::Unreadable {
                record: "key set",
                reason: "unlock parameters outside their bounds".to_owned(),
            })
        }
    }

    /// The salt.
    #[must_use]
    pub fn salt(&self) -> &[u8] {
        &self.salt
    }

    /// Memory, in KiB.
    #[must_use]
    pub const fn memory_kib(&self) -> u32 {
        self.memory_kib
    }

    /// Passes.
    #[must_use]
    pub const fn passes(&self) -> u32 {
        self.passes
    }

    /// Lanes.
    #[must_use]
    pub const fn lanes(&self) -> u32 {
        self.lanes
    }
}

/// The public half of a member's key set, as the directory lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKeys {
    /// The member it belongs to.
    pub account: AccountId,
    /// The id wraps to it are named by.
    pub key_id: KeyId,
    /// X25519: what record and group keys are wrapped to.
    pub agreement_public: Vec<u8>,
    /// Ed25519: what the member's statements and revisions are verified with.
    pub signing_public: Vec<u8>,
}

/// A key set as the member's browser hands it over; whose it is comes from the session, never from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewKeys {
    key_id: KeyId,
    agreement_public: Vec<u8>,
    signing_public: Vec<u8>,
    kdf: Kdf,
    wrapped: Vec<u8>,
}

impl NewKeys {
    /// A key set, each part checked against its shape and ceiling.
    ///
    /// # Errors
    ///
    /// When a public key is not 32 bytes or the wrapped half is empty or above its ceiling.
    pub fn new(
        key_id: KeyId,
        agreement_public: Vec<u8>,
        signing_public: Vec<u8>,
        kdf: Kdf,
        wrapped: Vec<u8>,
    ) -> Result<Self> {
        let shaped = agreement_public.len() == PUBLIC_KEY_BYTES
            && signing_public.len() == PUBLIC_KEY_BYTES
            && !wrapped.is_empty()
            && wrapped.len() <= MAX_WRAPPED_KEYS_BYTES;
        if shaped {
            Ok(Self {
                key_id,
                agreement_public,
                signing_public,
                kdf,
                wrapped,
            })
        } else {
            Err(Error::Unreadable {
                record: "key set",
                reason: "a part outside its shape or ceiling".to_owned(),
            })
        }
    }
}

/// A member's own key set: the public half, the unlock derivation, and the private half wrapped under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySet {
    /// The public half.
    pub public: PublicKeys,
    /// How the browser derives the key the private half is wrapped under.
    pub kdf: Kdf,
    wrapped: Vec<u8>,
}

impl KeySet {
    /// `new` as `account`'s key set.
    pub(crate) fn of(account: AccountId, new: NewKeys) -> Self {
        Self {
            public: PublicKeys {
                account,
                key_id: new.key_id,
                agreement_public: new.agreement_public,
                signing_public: new.signing_public,
            },
            kdf: new.kdf,
            wrapped: new.wrapped,
        }
    }

    /// The private half, wrapped under the unlock key; the server never opens it.
    #[must_use]
    pub fn wrapped(&self) -> &[u8] {
        &self.wrapped
    }
}

fn fault(what: &str) -> Error {
    Error::Unreadable {
        record: "key set",
        reason: format!("{what} did not read back as written"),
    }
}

fn decoded(text: &str, what: &str) -> Result<Vec<u8>> {
    base64url::decode(text).map_err(|_| fault(what))
}

impl KeyEntity {
    pub(crate) fn into_public(self) -> Result<PublicKeys> {
        Ok(PublicKeys {
            account: AccountId::parse(&self.account).map_err(|_| fault("account"))?,
            key_id: KeyId::parse(&self.key_id).map_err(|_| fault("key id"))?,
            agreement_public: decoded(&self.agreement_public, "agreement key")?,
            signing_public: decoded(&self.signing_public, "signing key")?,
        })
    }

    pub(crate) fn kdf(&self) -> Result<Kdf> {
        let number = |value: i64, what: &str| u32::try_from(value).map_err(|_| fault(what));
        Kdf::new(
            decoded(&self.kdf_salt, "salt")?,
            number(self.kdf_memory, "memory")?,
            number(self.kdf_passes, "passes")?,
            number(self.kdf_lanes, "lanes")?,
        )
        .map_err(|_| fault("unlock parameters"))
    }
}
