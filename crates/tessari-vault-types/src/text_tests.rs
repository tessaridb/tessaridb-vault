use super::{Email, Name, TextError};

#[test]
fn an_address_is_trimmed_and_lowercased() {
    let email = Email::parse("  Ann@Example.COM ");
    assert_eq!(
        email.map(|e| e.as_str().to_owned()),
        Ok("ann@example.com".to_owned())
    );
}

#[test]
fn shapes_that_are_not_addresses_are_refused() {
    for text in [
        "",
        "ann",
        "@example.com",
        "ann@",
        "ann@example",
        "a@b@c.d",
        "ann @x.io",
        "ann@.io",
        "ann@x.",
        "a\u{0}@x.io",
    ] {
        assert_eq!(Email::parse(text), Err(TextError::Email), "{text:?}");
    }
    let long = format!("{}@x.io", "a".repeat(250));
    assert_eq!(Email::parse(&long), Err(TextError::Email));
}

#[test]
fn a_name_is_trimmed_and_bounded() {
    assert_eq!(
        Name::parse(" Engineering ").map(|n| n.as_str().to_owned()),
        Ok("Engineering".to_owned())
    );
    assert_eq!(Name::parse("   "), Err(TextError::Name));
    assert_eq!(Name::parse(&"x".repeat(121)), Err(TextError::Name));
    assert!(Name::parse(&"я".repeat(120)).is_ok());
    assert_eq!(Name::parse("a\nb"), Err(TextError::Name));
}
