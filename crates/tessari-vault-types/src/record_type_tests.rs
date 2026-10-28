use super::RecordType;

#[test]
fn the_eleven_built_in_types_parse_and_anything_else_is_refused() {
    for id in [
        "login",
        "secure-note",
        "ssh-key",
        "file",
        "payment-card",
        "identity",
        "server",
        "database",
        "api-credential",
        "wireless-network",
        "software-license",
    ] {
        assert_eq!(RecordType::parse(id).map(|kind| kind.as_str()), Ok(id));
    }
    for text in ["", "Login", "password", "login ", "company:x"] {
        assert!(RecordType::parse(text).is_err(), "{text:?}");
    }
}
