//! The unlock key derivation: Argon2id over the unlock password, run in the browser.
//!
//! The bounds are checked by the browser against the parameters the server hands it, so a server cannot weaken a
//! user's derivation below the floor, nor make the browser exhaust its memory with an absurd cost.

/// Least memory an unlock derivation may use, in KiB: the OWASP Password Storage Cheat Sheet's first Argon2id line
/// (`m=47104, t=1, p=1`, retrieved 2026-10-28). The default is RFC 9106's second recommended option, above it.
pub const UNLOCK_KDF_MIN_MEMORY_KIB: u32 = 47_104;

/// Most memory an unlock derivation may use, in KiB: RFC 9106's first recommended option (2 GiB). Higher is refused
/// so stored parameters cannot ask a browser for more than any device can give.
pub const UNLOCK_KDF_MAX_MEMORY_KIB: u32 = 2_097_152;

/// Fewest passes an unlock derivation may make.
pub const UNLOCK_KDF_MIN_PASSES: u32 = 1;

/// Most passes an unlock derivation may make; beyond this an unlock takes longer than anyone waits.
pub const UNLOCK_KDF_MAX_PASSES: u32 = 10;

/// Fewest lanes an unlock derivation may use.
pub const UNLOCK_KDF_MIN_LANES: u32 = 1;

/// Most lanes an unlock derivation may use: RFC 9106's options use four, and the browser computes them in turn.
pub const UNLOCK_KDF_MAX_LANES: u32 = 8;

/// Bytes of salt per user: RFC 9106 §4, "a length of 128 bits is sufficient for all applications".
pub const UNLOCK_SALT_BYTES: usize = 16;

/// Bytes of the master key the derivation produces.
pub const MASTER_KEY_BYTES: usize = 32;

/// Longest unlock password accepted, in bytes; it bounds the work an input can cause and is far past any passphrase.
pub const UNLOCK_PASSWORD_MAX_BYTES: usize = 1024;
