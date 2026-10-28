//! The browser-side cryptography of TessariDB Vault: key derivation from the unlock password. Compiled for the server
//! tests and to WebAssembly for the panel; no I/O and no runtime.

mod error;

pub use error::{Error, Result};
