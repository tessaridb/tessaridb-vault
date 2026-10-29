//! The text a sign-in or an enrolment link carries before a session exists: identifiers, an address, base64url.
#![no_main]

use tessari_vault_types::{AccountId, Email, base64url};

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        if let Ok(id) = AccountId::parse(text) {
            assert_eq!(
                id.as_str(),
                text,
                "an identifier is the text it was parsed from"
            );
        }
        drop(Email::parse(text));
        drop(base64url::decode(text));
    }
});
