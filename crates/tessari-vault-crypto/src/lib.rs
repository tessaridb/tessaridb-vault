//! The browser-side cryptography of TessariDB Vault: key derivation from the unlock password. Compiled for the
//! server's tests and to WebAssembly for the panel; no I/O and no runtime.

mod error;
mod unlock;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use error::{Error, Result};
pub use unlock::{MasterKey, derive_master_key};
