use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair as _};

use super::{Act, DropRefused, statement, verify};

const COMPANY: &str = "0123456789abcdef0123456789abcdef";

fn signer() -> Ed25519KeyPair {
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).expect("a key");
    Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).expect("readable")
}

/// A stored signature: [version] ‖ [0x04] ‖ the 64 bytes, as the browser stores every signature.
fn signed(key: &Ed25519KeyPair, bytes: &[u8]) -> Vec<u8> {
    let mut stored = vec![1, 0x04];
    stored.extend_from_slice(key.sign(bytes).as_ref());
    stored
}

#[test]
fn the_statement_is_labelled_versioned_and_names_the_act_the_company_and_the_time() {
    let bytes = statement(Act::Request, COMPANY, 1_793_300_000);
    let mut expected = b"tessaridb-vault/v1/company-drop".to_vec();
    expected.extend_from_slice(&[1, 1]);
    expected.extend_from_slice(&32_u32.to_be_bytes());
    expected.extend_from_slice(COMPANY.as_bytes());
    expected.extend_from_slice(&1_793_300_000_u64.to_be_bytes());
    assert_eq!(bytes, expected);
    assert_ne!(
        statement(Act::Cancel, COMPANY, 1_793_300_000),
        bytes,
        "a cancel is not a request"
    );
}

#[test]
fn a_statement_signed_by_the_key_verifies_and_nothing_else_does() {
    let key = signer();
    let bytes = statement(Act::Request, COMPANY, 1_793_300_000);
    let signature = signed(&key, &bytes);
    let public = key.public_key().as_ref().to_vec();
    assert_eq!(verify(&public, &bytes, &signature), Ok(()));
    let other = signer().public_key().as_ref().to_vec();
    assert_eq!(
        verify(&other, &bytes, &signature),
        Err(DropRefused::BadSignature)
    );
    let cancel = statement(Act::Cancel, COMPANY, 1_793_300_000);
    assert_eq!(
        verify(&public, &cancel, &signature),
        Err(DropRefused::BadSignature),
        "a request's signature does not cancel"
    );
    let mut tampered = signature.clone();
    if let Some(last) = tampered.last_mut() {
        *last ^= 1;
    }
    assert_eq!(
        verify(&public, &bytes, &tampered),
        Err(DropRefused::BadSignature)
    );
    for malformed in [Vec::new(), vec![1, 0x04], signature[1..].to_vec()] {
        assert_eq!(
            verify(&public, &bytes, &malformed),
            Err(DropRefused::Malformed),
            "{malformed:?}"
        );
    }
}
