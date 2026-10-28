//! Key custody on the service host: the service key read from its file, and the sign-in password verifier keyed by
//! the pepper it derives.

mod passwords;
mod service_key;

pub use passwords::{PasswordCost, Passwords, Verdict};
pub use service_key::ServiceKey;
