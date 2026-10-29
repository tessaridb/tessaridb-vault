//! A `group_keys` row, the wraps beside it, and what they read as.

use tessari_vault_constants::{MAX_RECORD_WRAP_BYTES, PUBLIC_KEY_BYTES};
use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::{GroupId, KeyId, base64url};

use crate::{Error, Result};

/// One `group_keys` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GroupKeyEntity {
    pub(crate) company: String,
    pub(crate) key_id: String,
    pub(crate) group_public: String,
    pub(crate) rotate: bool,
}

impl FromRecord for GroupKeyEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            company: row.take("company")?,
            key_id: row.take("key_id")?,
            group_public: row.take("group_public")?,
            rotate: row.take("rotate")?,
        })
    }
}

/// The kind and company of a `groups` row, which decide the group's reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GroupHead {
    pub(crate) company: String,
    pub(crate) kind: String,
}

impl FromRecord for GroupHead {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            company: row.take("company")?,
            kind: row.take("kind")?,
        })
    }
}

fn invalid(what: &str) -> Error {
    Error::Unreadable {
        record: "group key",
        reason: format!("{what} outside its shape or ceiling"),
    }
}

/// The group's private key wrapped by an admin's browser to one member's key set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberWrap {
    key_id: KeyId,
    wrap: Vec<u8>,
}

impl MemberWrap {
    /// A wrap to the member key `key_id`, checked against its ceiling.
    ///
    /// # Errors
    ///
    /// When the wrap is empty or above the ceiling of a wrapped key.
    pub fn new(key_id: KeyId, wrap: Vec<u8>) -> Result<Self> {
        if wrap.is_empty() || wrap.len() > MAX_RECORD_WRAP_BYTES {
            return Err(invalid("a wrap"));
        }
        Ok(Self { key_id, wrap })
    }

    /// The member key it is wrapped to.
    #[must_use]
    pub const fn key_id(&self) -> &KeyId {
        &self.key_id
    }

    /// The wrapped group private key.
    #[must_use]
    pub fn wrap(&self) -> &[u8] {
        &self.wrap
    }
}

/// A group key as an admin's browser hands it over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGroupKey {
    key_id: KeyId,
    public: Vec<u8>,
    wraps: Vec<MemberWrap>,
}

impl NewGroupKey {
    /// A group key with its first wraps.
    ///
    /// # Errors
    ///
    /// When the public key is not 32 bytes.
    pub fn new(key_id: KeyId, public: Vec<u8>, wraps: Vec<MemberWrap>) -> Result<Self> {
        if public.len() != PUBLIC_KEY_BYTES {
            return Err(invalid("the public key"));
        }
        Ok(Self {
            key_id,
            public,
            wraps,
        })
    }

    pub(crate) const fn key_id(&self) -> &KeyId {
        &self.key_id
    }

    pub(crate) fn public(&self) -> &[u8] {
        &self.public
    }

    pub(crate) fn wraps(&self) -> &[MemberWrap] {
        &self.wraps
    }
}

/// A group's key as a member reads it: the public half, and the caller's own wrap of the private half if they hold one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupKey {
    /// The group.
    pub group: GroupId,
    /// The id record keys wrapped to it are named by.
    pub key_id: KeyId,
    /// X25519 public key.
    pub public: Vec<u8>,
    /// The caller's wrap of the private key, when they are in the reach and an admin has wrapped it to them.
    pub wrap: Option<Vec<u8>>,
    /// Whether a member left the group's reach, so an admin must key it again.
    pub rotation_due: bool,
}

/// A member's wrap of a group key, about to be taken off the group's vault record with the key that opens it.
pub(crate) struct Revocation {
    pub(crate) recipient: String,
    pub(crate) vault_key: tessari_vault_infrastructure::tessaridb::VaultKey,
}

fn fault(what: &str) -> Error {
    Error::Unreadable {
        record: "group key",
        reason: format!("{what} did not read back as written"),
    }
}

impl GroupKeyEntity {
    pub(crate) fn key_id(&self) -> Result<KeyId> {
        KeyId::parse(&self.key_id).map_err(|_| fault("key id"))
    }

    pub(crate) fn public(&self) -> Result<Vec<u8>> {
        base64url::decode(&self.group_public).map_err(|_| fault("public key"))
    }
}
