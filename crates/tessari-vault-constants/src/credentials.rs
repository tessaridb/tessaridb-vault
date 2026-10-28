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

/// The TOTP time step in seconds (RFC 6238 §4: the default 30 seconds every authenticator app uses).
pub const TOTP_STEP_SECS: u64 = 30;

/// The digits a TOTP code has (RFC 4226 §5.3: six).
pub const TOTP_DIGITS: usize = 6;

/// How many steps either side of the current one a code is accepted for: one, for a clock or a slow typist.
pub const TOTP_SKEW_STEPS: u64 = 1;

/// The length of a TOTP seed in bytes: 160 bits, the HMAC-SHA-1 block RFC 4226 §4 recommends.
pub const TOTP_SECRET_BYTES: usize = 20;

/// The fewest characters a sign-in password has: eight, because a second factor is always required with it (NIST SP
/// 800-63B-4 §3.1.1, "SHALL ≥8 when used only within MFA").
pub const PASSWORD_MIN_CHARS: usize = 8;

/// The most characters a sign-in password may have; long enough for any passphrase, short enough to bound the work.
pub const PASSWORD_MAX_CHARS: usize = 256;

/// How long an enrolment link can be used, in hours; a newer link for the same account replaces it.
pub const ENROLMENT_LIFETIME_HOURS: u32 = 24;

/// The length of an enrolment token in bytes: 256 bits, so its digest needs no salt and no slow hash.
pub const ENROLMENT_TOKEN_BYTES: usize = 32;

/// How long a sign-in challenge waits for its code, in minutes, after the password was right.
pub const SIGN_IN_CHALLENGE_MINUTES: u32 = 5;

/// Wrong codes a challenge takes before it ends; the person then starts again with the password.
pub const SIGN_IN_CODE_MISSES: u32 = 5;

/// Sign-in attempts per window for one address (the account key), known or not.
pub const SIGN_IN_PER_ACCOUNT: u32 = 10;

/// Sign-in attempts per window from one network address.
pub const SIGN_IN_PER_ADDRESS: u32 = 20;

/// Attempts per window with one address-and-password pair: a breached pair tried again and again.
pub const SIGN_IN_PER_PAIR: u32 = 3;
