use super::super::ServiceKey;
use super::super::service_key::tests::file;
use super::SigninKeys;

fn keys(byte: u8) -> SigninKeys {
    let key =
        ServiceKey::load(&file(&format!("{byte:02x}").repeat(32), 0o600)).expect("a key file");
    SigninKeys::new(&key).expect("keys")
}

#[test]
fn names_are_keyed_by_the_service_key_and_hold_no_address_or_password() {
    let (one, two) = (keys(1), keys(2));
    let email = "ada@example.com";
    assert_eq!(one.account(email), one.account(email), "one key, one name");
    assert_ne!(
        one.account(email),
        two.account(email),
        "another service key"
    );
    assert_ne!(one.account(email), one.account("bob@example.com"));
    let pair = one.pair(email, "correct horse");
    assert_eq!(pair.len(), 64);
    assert!(!pair.contains("ada") && !pair.contains("horse"));
    assert_ne!(pair, one.pair(email, "correct horsf"));
    assert_ne!(
        one.pair("ab", "c"),
        one.pair("a", "bc"),
        "lengths keep the parts apart"
    );
    assert_eq!(
        one.pair(email, "caf\u{e9}"),
        one.pair(email, "cafe\u{301}"),
        "the same characters"
    );
}
