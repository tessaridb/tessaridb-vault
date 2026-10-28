use tessari_vault_types::KdfParams;

use super::derive_master_key;

/// Argon2id at the unlock floor (RFC 9106 second option: m = 64 MiB, t = 3, p = 4). The expected tag was produced by
/// Node's `crypto.argon2Sync`, an implementation independent of this crate's, which reproduces the RFC 9106 §5.3
/// Argon2id vector (`0d640df5…e659`) on the same machine.
const FLOOR_VECTOR: &str = "853b272a44db1421c02962669a55eb0994f3cab385ed1c4c79253eee19bab49e";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn the_floor_parameters_reproduce_the_independent_vector() {
    let params = KdfParams::new(65_536, 3, 4).expect("the floor parameters are valid");
    let salt: [u8; 16] = core::array::from_fn(|index| u8::try_from(index).unwrap_or(0));
    let key = derive_master_key(b"correct horse battery staple", &salt, params).expect("derives");
    assert_eq!(hex(key.expose()), FLOOR_VECTOR);
}

#[test]
fn a_different_password_derives_a_different_key() {
    let params = KdfParams::new(65_536, 3, 4).expect("valid");
    let salt = [7_u8; 16];
    let first = derive_master_key(b"one", &salt, params).expect("derives");
    let second = derive_master_key(b"two", &salt, params).expect("derives");
    assert_ne!(first.expose(), second.expose());
}

#[test]
fn an_oversized_password_is_refused() {
    let params = KdfParams::new(65_536, 3, 4).expect("valid");
    let password = vec![b'x'; 1025];
    assert!(derive_master_key(&password, &[0; 16], params).is_err());
}

#[test]
fn the_key_does_not_print() {
    let params = KdfParams::new(65_536, 3, 4).expect("valid");
    let key = derive_master_key(b"secret", &[1; 16], params).expect("derives");
    assert_eq!(format!("{key:?}"), "MasterKey(redacted)");
}
