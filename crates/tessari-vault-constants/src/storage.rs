//! How the server reaches its store, and how long what it keeps there lives.

/// The most connections the server holds to the store at once.
pub const STORE_CONNECTIONS: usize = 16;

/// The first pause before a throttled store sign-in is tried again, in milliseconds; doubled each time up to
/// [`STORE_RETRY_MAX_PAUSE_MS`].
pub const STORE_RETRY_FIRST_PAUSE_MS: u64 = 2;

/// The longest pause between store sign-in attempts, in milliseconds.
pub const STORE_RETRY_MAX_PAUSE_MS: u64 = 64;

/// The kind of note a node attaches to a read that names a field its table does not declare.
pub const STORE_NOTE_UNDECLARED_FIELD: &str = "undeclared-field";

/// How many connections sign in to the store at once. The node verifies only so many passwords concurrently and
/// answers `throttled` beyond that.
pub const STORE_SIGNIN_CONCURRENCY: usize = 4;

/// How many times one new connection tries to sign in before the request fails.
pub const STORE_SIGNIN_ATTEMPTS: u32 = 6;

/// How many times a transaction the store answered `retry` is sent again.
pub const STORE_RETRY_ATTEMPTS: u32 = 8;

/// The length of a session token, in bytes before encoding (256 bits).
pub const SESSION_TOKEN_BYTES: usize = 32;

/// How long a session lives without being renewed, in minutes.
pub const SESSION_LIFETIME_MINUTES: u32 = 720;

/// Requests one address may make in one window.
pub const RATE_PER_ADDRESS: u32 = 300;

/// Requests one account may make in one window.
pub const RATE_PER_ACCOUNT: u32 = 600;

/// The longest reason a caller may state for a change, in characters: a sentence or two, recorded on the audit trail.
pub const REASON_MAX_CHARS: usize = 500;

/// The length of a rate window, in seconds.
pub const RATE_WINDOW_SECONDS: u64 = 60;
