//! The WebAssembly boundary: three functions over the module's own memory, so the panel needs no generated glue code
//! and no third-party script. The panel allocates a buffer, copies the password and salt in, derives into another
//! buffer, reads the key, and frees every buffer — `vault_free` clears the bytes before releasing them.

use tessari_vault_constants::{MASTER_KEY_BYTES, UNLOCK_SALT_BYTES};
use tessari_vault_types::KdfParams;
use zeroize::Zeroize;

use crate::{Error, derive_master_key};

/// `vault_derive` succeeded and the key is in the output buffer.
const DERIVED: i32 = 0;
/// The parameters are outside the bounds.
const REFUSED_PARAMS: i32 = 1;
/// The password is too long.
const REFUSED_PASSWORD: i32 = 2;
/// Argon2id itself refused.
const FAILED: i32 = 3;

/// Allocate `len` bytes the panel may write into; returns null when `len` is zero.
#[unsafe(no_mangle)]
pub extern "C" fn vault_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return core::ptr::null_mut();
    }
    let mut buffer = core::mem::ManuallyDrop::new(vec![0_u8; len]);
    buffer.as_mut_ptr()
}

/// Clear and release a buffer `vault_alloc` returned with the same `len`.
///
/// # Safety
///
/// `ptr` must come from `vault_alloc(len)` with the same `len`, and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vault_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    // SAFETY: the caller hands back a pointer `vault_alloc(len)` produced from a `Vec` whose length and capacity are
    // both `len`, and does not use it again, so rebuilding that `Vec` here owns the allocation exactly once.
    let mut buffer = unsafe { Vec::from_raw_parts(ptr, len, len) };
    buffer.zeroize();
}

/// Derive the master key into `out` (32 bytes) from the password at `password` (`password_len` bytes) and the salt
/// at `salt` (16 bytes). Returns 0 on success, 1 for parameters out of bounds, 2 for a password too long, 3 when
/// Argon2id refused.
///
/// # Safety
///
/// `password` must be readable for `password_len` bytes, `salt` for 16 and `out` writable for 32, each inside a
/// buffer from `vault_alloc`, and `out` must not overlap the inputs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vault_derive(
    password: *const u8,
    password_len: usize,
    salt: *const u8,
    memory_kib: u32,
    passes: u32,
    lanes: u32,
    out: *mut u8,
) -> i32 {
    let Ok(params) = KdfParams::new(memory_kib, passes, lanes) else {
        return REFUSED_PARAMS;
    };
    // SAFETY: the caller guarantees each region lies inside a live buffer of at least the stated length.
    let password = unsafe { core::slice::from_raw_parts(password, password_len) };
    // SAFETY: as above, `salt` is readable for exactly `UNLOCK_SALT_BYTES` bytes.
    let salt = unsafe { &*salt.cast::<[u8; UNLOCK_SALT_BYTES]>() };
    // SAFETY: as above, `out` is writable for `MASTER_KEY_BYTES` bytes and overlaps neither input.
    let out = unsafe { &mut *out.cast::<[u8; MASTER_KEY_BYTES]>() };
    match derive_master_key(password, salt, params) {
        Ok(key) => {
            out.copy_from_slice(key.expose());
            DERIVED
        }
        Err(Error::PasswordTooLong) => REFUSED_PASSWORD,
        Err(Error::Derivation) => FAILED,
    }
}
