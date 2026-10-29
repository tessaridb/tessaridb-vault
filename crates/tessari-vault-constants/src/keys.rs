//! Member key sets and the vouching chain, as the server bounds them; it reads neither.

/// An X25519 or Ed25519 public key.
pub const PUBLIC_KEY_BYTES: usize = 32;

/// The largest wrapped key set accepted: a member's two private keys sealed under the unlock key (format 0x03,
/// 62 bytes each), with room for the escrow wraps a later version adds.
pub const MAX_WRAPPED_KEYS_BYTES: usize = 4_096;

/// The most key sets one directory answers: one per member, and no company holds more members.
pub const KEY_LIST_MAX: i64 = 10_000;

/// The largest vouching statement accepted (a member statement is under 200 bytes).
pub const MAX_STATEMENT_BYTES: usize = 1_024;

/// A signature over a statement as the browser stores it: `[version][0x04][Ed25519 64]`.
pub const STATEMENT_SIGNATURE_BYTES: usize = 66;

/// The longest chain a company keeps. A chain is read whole or not at all, because a client offered a prefix cannot
/// tell it from a chain whose revocations were left off.
pub const CHAIN_MAX: u64 = 10_000;

/// How long a recovery request stays open after the member asks: 72 hours, then a new request is made.
pub const RECOVERY_REQUEST_SECONDS: i64 = 259_200;

/// The most open recovery requests one listing answers.
pub const RECOVERY_REQUEST_LIST_MAX: i64 = 1_000;

/// How long a confirmed company deletion waits, read-only and cancellable by an owner, before it may be carried out:
/// seven days.
pub const COMPANY_DROP_WAIT_SECONDS: i64 = 604_800;
