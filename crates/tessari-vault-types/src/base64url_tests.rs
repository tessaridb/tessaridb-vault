use super::{decode, encode};

#[test]
fn encodes_the_rfc_4648_vectors_without_padding() {
    // RFC 4648 §10 test vectors, with the padding the standard alphabet would add removed.
    for (input, expected) in [
        ("", ""),
        ("f", "Zg"),
        ("fo", "Zm8"),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg"),
        ("fooba", "Zm9vYmE"),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(encode(input.as_bytes()), expected, "{input:?}");
    }
}

#[test]
fn uses_the_url_safe_characters_for_the_last_two_slots() {
    // 0xfb 0xff encodes to the 62nd and 63rd characters, which differ between the two alphabets.
    assert_eq!(encode(&[0xfb, 0xff]), "-_8");
}

#[test]
fn decodes_the_rfc_4648_vectors_and_the_url_safe_slots() {
    for (expected, input) in [
        ("", ""),
        ("f", "Zg"),
        ("fo", "Zm8"),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg"),
        ("fooba", "Zm9vYmE"),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(
            decode(input).as_deref(),
            Ok(expected.as_bytes()),
            "{input:?}"
        );
    }
    assert_eq!(decode("-_8").as_deref(), Ok(&[0xfb, 0xff][..]));
}

#[test]
fn decoding_refuses_padding_foreign_characters_an_impossible_length_and_stray_bits() {
    for input in [
        "Zg==",  // padding
        "Zm9v+", // the standard alphabet
        "Zm9vY", // five characters cannot end a group
        "Zh",    // `h` sets bits that `f` alone cannot produce: two spellings of one value
        "Zm9",   // likewise for a two-byte tail
    ] {
        assert!(decode(input).is_err(), "{input:?}");
    }
}
