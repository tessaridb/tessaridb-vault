//! The service key and the sign-in password verifier.

/// The service key's length in bytes: one HKDF-SHA-256 input of 256 bits (key-hierarchy worksheet, K_svc).
pub const SERVICE_KEY_BYTES: usize = 32;

/// The least Argon2id memory a sign-in verifier is made with, in KiB: RFC 9106's second recommended option (64 MiB),
/// the family's floor, above every OWASP Password Storage Cheat Sheet profile.
pub const PASSWORD_HASH_MEMORY_KIB_FLOOR: u32 = 65_536;

/// The fewest Argon2id passes a sign-in verifier is made with: RFC 9106's second recommended option.
pub const PASSWORD_HASH_ITERATIONS_FLOOR: u32 = 3;

/// The fewest Argon2id lanes: one, because a server verifies many sign-ins at once and lanes cost threads.
pub const PASSWORD_HASH_PARALLELISM_FLOOR: u32 = 1;

/// The Argon2id tag length in bytes (RFC 9106: 256-bit tag).
pub const PASSWORD_HASH_OUTPUT_BYTES: usize = 32;

/// The salt length in bytes (RFC 9106: 128 bits is sufficient for all applications).
pub const PASSWORD_SALT_BYTES: usize = 16;

/// How many verifications run at once when the configuration says nothing: each holds the configured memory.
pub const PASSWORD_CONCURRENCY_DEFAULT: usize = 4;
