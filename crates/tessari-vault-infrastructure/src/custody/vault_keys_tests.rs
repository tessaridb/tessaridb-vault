use super::VaultKeys;
use crate::custody::ServiceKey;
use crate::custody::service_key::tests::file;

/// The key whose bytes count from 00 up to 1f.
const KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

/// KV(c) for two companies, from Python's `hmac` (HKDF-SHA256, empty salt): KVM under
/// `tessaridb-vault/v1/db-vault`, then KV(c) from KVM with the company id as the info.
const COMPANY_A: &str = "0123456789abcdef0123456789abcdef";
const COMPANY_A_KEY_HEX: &str = "9a1467218bb44fb2dee4f4c558c56238141b10933f42ee44bfa44beb87d71e60";
const COMPANY_B: &str = "fedcba9876543210fedcba9876543210";
const COMPANY_B_KEY_HEX: &str = "81b8c4133e7098285cd63d35ad70edf6b59eef9f47613562bfa300c48edaecd4";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn keys() -> VaultKeys {
    let service = ServiceKey::load(&file(KEY_HEX, 0o600)).expect("a key file");
    VaultKeys::new(&service).expect("derived")
}

#[test]
fn each_company_key_is_the_independent_two_step_hkdf_of_the_service_key() {
    let keys = keys();
    for (company, expected) in [
        (COMPANY_A, COMPANY_A_KEY_HEX),
        (COMPANY_B, COMPANY_B_KEY_HEX),
    ] {
        let key = keys.company(company).expect("derived");
        assert_eq!(hex(key.expose()), expected, "{company}");
    }
}

#[test]
fn the_keys_never_print() {
    assert_eq!(format!("{:?}", keys()), "VaultKeys(<redacted>)");
}
