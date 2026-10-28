use super::RecordType;
use crate::Roles;

#[test]
fn the_built_in_types_parse_and_anything_else_is_refused() {
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
        "record-type",
    ] {
        assert_eq!(
            RecordType::parse(id)
                .map(|kind| kind.to_string())
                .as_deref(),
            Ok(id)
        );
    }
    for text in [
        "",
        "Login",
        "password",
        "login ",
        "company:x",
        "company:",
        "Company:0123456789abcdef0123456789abcdef",
    ] {
        assert!(RecordType::parse(text).is_err(), "{text:?}");
    }
}

#[test]
fn a_company_type_is_written_as_its_prefix_and_id_and_carries_no_roles_of_its_own() {
    let text = "company:0123456789abcdef0123456789abcdef";
    let kind = RecordType::parse(text).expect("a company type");
    assert_eq!(kind.to_string(), text);
    assert_eq!(kind.built_in_roles(), None);
}

#[test]
fn only_managers_may_keep_a_type_definition() {
    let definition = RecordType::parse("record-type").expect("built in");
    assert!(definition.is_definition());
    assert_eq!(definition.built_in_roles(), Some(Roles::MANAGERS));
    let login = RecordType::parse("login").expect("built in");
    assert!(!login.is_definition());
    assert_eq!(login.built_in_roles(), Some(Roles::ALL));
}
