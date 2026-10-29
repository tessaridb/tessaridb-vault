use super::VaultKeys;
use crate::Error;
use crate::custody::KeyRing;
use crate::custody::service_key::tests::file;

/// The key whose bytes count from 00 up to 1f.
const KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

/// A second epoch's key.
const SECOND_KEY_HEX: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

/// KV(c) for two companies, from Python's `hmac` (HKDF-SHA256, empty salt): KVM under
/// `tessaridb-vault/v1/db-vault`, then KV(c) from KVM with the company id as the info.
const COMPANY_A: &str = "0123456789abcdef0123456789abcdef";
const COMPANY_A_KEY_HEX: &str = "9a1467218bb44fb2dee4f4c558c56238141b10933f42ee44bfa44beb87d71e60";
const COMPANY_B: &str = "fedcba9876543210fedcba9876543210";
const COMPANY_B_KEY_HEX: &str = "81b8c4133e7098285cd63d35ad70edf6b59eef9f47613562bfa300c48edaecd4";

/// KV(A) the same independent way, from the second epoch's key.
const COMPANY_A_SECOND_KEY_HEX: &str =
    "4a4c1b232a5dea75052bb9d98f264ceef4931c84f22e05a92334c4a2c26c90cc";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn keys(text: &str) -> VaultKeys {
    let ring = KeyRing::load(&file(text, 0o600)).expect("a key file");
    VaultKeys::new(&ring).expect("derived")
}

#[test]
fn each_company_key_is_the_independent_two_step_hkdf_of_the_service_key() {
    let keys = keys(KEY_HEX);
    for (company, expected) in [
        (COMPANY_A, COMPANY_A_KEY_HEX),
        (COMPANY_B, COMPANY_B_KEY_HEX),
    ] {
        let key = keys.company(1, company).expect("derived");
        assert_eq!(hex(key.expose()), expected, "{company}");
    }
}

#[test]
fn each_epoch_derives_from_its_own_key_and_the_newest_is_current() {
    let keys = keys(&format!("1 {KEY_HEX}\n2 {SECOND_KEY_HEX}\n"));
    assert_eq!(keys.current_epoch(), 2);
    for (epoch, expected) in [(1, COMPANY_A_KEY_HEX), (2, COMPANY_A_SECOND_KEY_HEX)] {
        let key = keys.company(epoch, COMPANY_A).expect("derived");
        assert_eq!(hex(key.expose()), expected, "epoch {epoch}");
    }
}

#[test]
fn an_epoch_the_ring_does_not_hold_is_refused_rather_than_replaced() {
    let keys = keys(&format!("2 {SECOND_KEY_HEX}\n"));
    for epoch in [0, 1, 3] {
        match keys.company(epoch, COMPANY_A) {
            Err(Error::Custody { .. }) => {}
            other => panic!("epoch {epoch}: {:?}", other.map(|_| "a key")),
        }
    }
}

#[test]
fn the_keys_never_print() {
    assert_eq!(format!("{:?}", keys(KEY_HEX)), "VaultKeys(<redacted>)");
}
