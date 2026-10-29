//! Both passkey ceremonies and the challenge lookup, on response parts an unsigned-in browser chooses.
#![no_main]

use tessari_vault_core::webauthn::{Expected, assert, challenge_of, register};

const EXPECTED: Expected<'static> = Expected {
    rp_id: "vault.example",
    origins: &["https://vault.example"],
    challenge: b"0123456789abcdef0123456789abcdef",
    user_verification: true,
};

/// The next part: a two-byte length, then that many bytes (fewer when the input runs out).
fn part<'a>(data: &mut &'a [u8]) -> &'a [u8] {
    let Some((head, rest)) = data.split_first_chunk::<2>() else {
        return &[];
    };
    let (taken, rest) = rest.split_at(usize::from(u16::from_le_bytes(*head)).min(rest.len()));
    *data = rest;
    taken
}

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let mut data = data;
    let client_data = part(&mut data);
    let authenticator_data = part(&mut data);
    let signature = part(&mut data);
    let public_key = data;
    drop(challenge_of(client_data));
    drop(register(client_data, authenticator_data, &EXPECTED));
    let _ = assert(
        client_data,
        authenticator_data,
        signature,
        public_key,
        7,
        &EXPECTED,
    );
});
