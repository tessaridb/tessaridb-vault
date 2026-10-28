//! Records as the server holds them: sealed bytes it bounds, and never reads.

/// The largest sealed record body accepted, decoded: the browser's record document ceilings (64 sections, 512
/// fields, 65 536 characters per text) sealed, with room to spare.
pub const MAX_RECORD_BODY_BYTES: usize = 1_048_576;

/// An Ed25519 signature over a revision.
pub const RECORD_SIGNATURE_BYTES: usize = 64;

/// The largest wrapped record key accepted (format 0x02 is 82 bytes).
pub const MAX_RECORD_WRAP_BYTES: usize = 256;

/// The most records one listing answers.
pub const RECORD_LIST_MAX: i64 = 1_000;
