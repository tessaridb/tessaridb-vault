use super::{AccountId, CompanyId, IdError};

#[test]
fn random_bytes_spell_thirty_two_lowercase_hex_characters() {
    let id = AccountId::from_random([0xab; 16]);
    assert_eq!(id.as_str(), "abababababababababababababababab");
    assert_eq!(AccountId::parse(id.as_str()), Ok(id));
}

#[test]
fn anything_else_is_not_an_identifier() {
    for text in [
        "",
        "ABABABABABABABABABABABABABABABAB",
        "abababababababababababababababa",
        "abababababababababababababababab0",
        "gbababababababababababababababab",
        "../ababababababababababababababab",
        "ababababababababababababababab' ",
    ] {
        assert_eq!(CompanyId::parse(text), Err(IdError), "{text:?}");
    }
}

#[test]
fn a_company_and_its_own_group_share_one_identifier() {
    let company = CompanyId::from_random([7; 16]);
    assert_eq!(company.group().as_str(), company.as_str());
}
