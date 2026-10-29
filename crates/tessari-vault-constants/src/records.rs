//! Records as the server holds them: sealed bytes it bounds, and never reads.

/// The largest sealed record body accepted, decoded: the browser's record document ceilings (64 sections, 512
/// fields, 65 536 characters per text) sealed, with room to spare.
pub const MAX_RECORD_BODY_BYTES: usize = 1_048_576;

/// A signature over a revision as the browser stores it: `[version][0x04][Ed25519 64]`.
pub const RECORD_SIGNATURE_BYTES: usize = 66;

/// The largest wrapped record key accepted (format 0x02 is 82 bytes).
pub const MAX_RECORD_WRAP_BYTES: usize = 256;

/// The most records one listing answers.
pub const RECORD_LIST_MAX: i64 = 1_000;

/// The largest file kept in the company's records vault: one sealed chunk (a 57-byte header, 64 KiB and its tag).
pub const MAX_VAULT_FILE_BYTES: u64 = 65_609;

/// The largest file accepted: the browser's 256 MiB ceiling sealed (a 57-byte header and 4 096 chunks of 64 KiB, each
/// with its 16-byte tag).
pub const MAX_FILE_BYTES: u64 = 268_501_049;

/// The most bytes one upload or download of a file part carries.
pub const FILE_PART_MAX_BYTES: u64 = 4_194_304;
