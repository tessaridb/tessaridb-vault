use super::{SessionToken, digest_key};

#[test]
fn only_the_shape_of_a_token_has_a_key() {
    let token = "ab".repeat(32);
    let key = digest_key(&token).expect("a token's shape");
    assert_eq!(key.len(), 64);
    assert_ne!(key, token, "the store key is the digest, never the token");
    for bad in [
        "",
        "AB",
        &"ab".repeat(31),
        &"AB".repeat(32),
        &"zz".repeat(32),
        &format!("{}'", "a".repeat(63)),
    ] {
        assert_eq!(digest_key(bad), None, "{bad:?}");
    }
}

#[test]
fn a_token_never_prints() {
    let token = SessionToken("ab".repeat(32));
    assert_eq!(format!("{token:?}"), "SessionToken(<redacted>)");
}
