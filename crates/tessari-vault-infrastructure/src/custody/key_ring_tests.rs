use super::KeyRing;
use crate::Error;
use crate::custody::ServiceKey;
use crate::custody::service_key::tests::file;

const ONE: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const TWO: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

fn same(left: &ServiceKey, right: &ServiceKey) -> bool {
    left.pepper().expect("derived")[..] == right.pepper().expect("derived")[..]
}

#[test]
fn a_file_of_one_key_is_epoch_one_and_current() {
    let ring = KeyRing::load(&file(&format!("{ONE}\n"), 0o600)).expect("a ring");
    let single = ServiceKey::load(&file(ONE, 0o600)).expect("a key");
    assert_eq!(ring.current_epoch(), 1);
    assert!(same(ring.current(), &single));
    assert!(same(ring.epoch(1).expect("epoch 1"), &single));
    assert!(ring.epoch(2).is_none());
}

#[test]
fn a_ring_names_each_epoch_and_the_highest_is_current() {
    let ring = KeyRing::load(&file(&format!("1 {ONE}\n2 {TWO}\n"), 0o400)).expect("a ring");
    assert_eq!(ring.current_epoch(), 2);
    assert!(same(
        ring.current(),
        &ServiceKey::load(&file(TWO, 0o600)).expect("a key")
    ));
    assert!(same(
        ring.epoch(1).expect("epoch 1"),
        &ServiceKey::load(&file(ONE, 0o600)).expect("a key")
    ));
    assert_eq!(ring.epochs(), vec![1, 2]);
}

#[test]
fn a_ring_out_of_shape_or_readable_by_others_is_refused_by_name() {
    for (text, mode) in [
        (format!("1 {ONE}\n1 {TWO}\n"), 0o600),
        (format!("0 {ONE}\n"), 0o600),
        (format!("2 {TWO}\n1 {ONE}\n"), 0o600),
        (format!("1 {ONE}\n2 {}\n", &TWO[..62]), 0o600),
        (format!("1 {ONE}\nx {TWO}\n"), 0o600),
        (String::new(), 0o600),
        (format!("1 {ONE}\n"), 0o640),
    ] {
        match KeyRing::load(&file(&text, mode)) {
            Err(Error::InvalidConfig { field, .. }) => {
                assert_eq!(field, "TESSARIDB_VAULT_SERVICE_KEY_FILE", "{text:?}");
            }
            other => panic!("{text:?} at {mode:o}: {other:?}"),
        }
    }
}

#[test]
fn the_ring_never_prints_a_key() {
    let ring = KeyRing::load(&file(&format!("1 {ONE}\n2 {TWO}\n"), 0o600)).expect("a ring");
    let printed = format!("{ring:?}");
    assert!(
        !printed.contains(&ONE[..16]) && !printed.contains(&TWO[..16]),
        "{printed}"
    );
}
