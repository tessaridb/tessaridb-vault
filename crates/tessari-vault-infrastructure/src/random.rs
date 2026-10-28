//! Random bytes from the operating system, for identifiers and session tokens.

use ring::rand::{SecureRandom, SystemRandom};

use crate::{Error, Result};

/// `N` bytes from the operating system's random source.
///
/// # Errors
///
/// [`Error::Randomness`] when the source fails; nothing falls back to a weaker one.
pub fn bytes<const N: usize>() -> Result<[u8; N]> {
    let mut drawn = [0_u8; N];
    SystemRandom::new()
        .fill(&mut drawn)
        .map_err(|_| Error::Randomness)?;
    Ok(drawn)
}
