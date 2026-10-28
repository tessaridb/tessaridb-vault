//! Every number with a meaning in TessariDB Vault, typed and documented with its unit and its reason.

mod unlock;

pub use unlock::{
    MASTER_KEY_BYTES, UNLOCK_KDF_MAX_LANES, UNLOCK_KDF_MAX_MEMORY_KIB, UNLOCK_KDF_MAX_PASSES,
    UNLOCK_KDF_MIN_LANES, UNLOCK_KDF_MIN_MEMORY_KIB, UNLOCK_KDF_MIN_PASSES,
    UNLOCK_PASSWORD_MAX_BYTES, UNLOCK_SALT_BYTES,
};
