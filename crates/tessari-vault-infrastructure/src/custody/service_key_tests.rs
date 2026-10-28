use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::ServiceKey;
use crate::Error;

static FILES: AtomicU32 = AtomicU32::new(0);

/// The key whose bytes count from 00 up to 1f, as the file holds it.
const KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

/// HKDF-SHA-256 of that key under `tessaridb-vault/v1/pepper`, computed with Python's `hmac` and `hashlib`, not ring.
const PEPPER_HEX: &str = "54f9546a33ae886c6b4013b86f2949f053c4a5800d56a5365206caabf253ddf6";

/// The same under `tessaridb-vault/v1/totp`, computed the same independent way.
const TOTP_KEY_HEX: &str = "64149a1d501c3b4cc680a3a78bb73ca5bc6e22787485c1a35bb50380f95d5ac4";

/// The same under `tessaridb-vault/v1/sign-in`.
const SIGNIN_KEY_HEX: &str = "bd881b8421cc435299bb81e607f688754021c592755d59d546924765eb09180b";

/// A key file holding `text` with `mode`, in a path nobody else uses.
pub(in crate::custody) fn file(text: &str, mode: u32) -> PathBuf {
    let turn = FILES.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "vault-service-key-test-{}-{turn}",
        std::process::id()
    ));
    std::fs::write(&path, text).expect("written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).expect("mode set");
    path
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn each_derived_key_is_the_independent_hkdf_of_the_key_under_its_label() {
    for text in [
        KEY_HEX.to_owned(),
        format!("{KEY_HEX}\n"),
        KEY_HEX.to_uppercase(),
    ] {
        let key = ServiceKey::load(&file(&text, 0o600)).expect("a key file");
        assert_eq!(hex(key.pepper().expect("derived").as_slice()), PEPPER_HEX);
        assert_eq!(
            hex(key.totp_key().expect("derived").as_slice()),
            TOTP_KEY_HEX
        );
        assert_eq!(
            hex(key.signin_key().expect("derived").as_slice()),
            SIGNIN_KEY_HEX
        );
    }
    let other = ServiceKey::load(&file(&"ff".repeat(32), 0o400)).expect("a key file");
    assert_ne!(hex(other.pepper().expect("derived").as_slice()), PEPPER_HEX);
}

#[test]
fn a_file_of_another_shape_or_readable_by_others_is_refused_by_name() {
    let shape = "64 hexadecimal characters, as `openssl rand -hex 32` writes";
    let others = "readable by group or others; it must be mode 600 or 400";
    for (path, reason) in [
        (file(KEY_HEX, 0o640), others),
        (file(KEY_HEX, 0o604), others),
        (file(&KEY_HEX[2..], 0o600), shape),
        (file(&format!("{KEY_HEX}00"), 0o600), shape),
        (file(&format!("{KEY_HEX}\n\n"), 0o600), shape),
        (file(&format!("{}zz", &KEY_HEX[2..]), 0o600), shape),
        (file(&format!(" {}", &KEY_HEX[1..]), 0o600), shape),
        (
            std::env::temp_dir().join("vault-service-key-test-absent"),
            "the file cannot be read",
        ),
    ] {
        match ServiceKey::load(&path) {
            Err(Error::InvalidConfig { field, reason: got }) => {
                assert_eq!(field, "TESSARIDB_VAULT_SERVICE_KEY_FILE");
                assert_eq!(got, reason, "{path:?}");
            }
            other => panic!("{path:?} loaded: {other:?}"),
        }
    }
}

#[test]
fn the_key_never_prints() {
    let key = ServiceKey::load(&file(KEY_HEX, 0o600)).expect("a key file");
    assert_eq!(format!("{key:?}"), "ServiceKey(<redacted>)");
}

/// KV(c) for two companies, from Python's `hmac` (HKDF-SHA256, empty salt, info = label ‖ company id).
const COMPANY_A: &str = "0123456789abcdef0123456789abcdef";
const COMPANY_A_VAULT_KEY_HEX: &str =
    "4ce9366fa153ec4f19ba51211b95da7a6d5d4e4c42db526cef7e7c783007fd4f";
const COMPANY_B: &str = "fedcba9876543210fedcba9876543210";
const COMPANY_B_VAULT_KEY_HEX: &str =
    "6b61b6ae79dfeda63aa6695d5d405a83542e243f8fdc2b99b7012cb56a2782ee";

#[test]
fn each_company_vault_key_is_the_independent_hkdf_of_the_key_and_its_company() {
    let key = ServiceKey::load(&file(KEY_HEX, 0o600)).expect("a key file");
    assert_eq!(
        hex(key
            .company_vault_key(COMPANY_A)
            .expect("derived")
            .as_slice()),
        COMPANY_A_VAULT_KEY_HEX
    );
    assert_eq!(
        hex(key
            .company_vault_key(COMPANY_B)
            .expect("derived")
            .as_slice()),
        COMPANY_B_VAULT_KEY_HEX
    );
}
