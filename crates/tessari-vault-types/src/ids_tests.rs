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

/// A fixed-seed xorshift, so a failing case is reproducible from the iteration number alone.
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn bytes(&mut self) -> [u8; 16] {
        let mut bytes = [0; 16];
        for chunk in bytes.chunks_mut(8) {
            chunk.copy_from_slice(&self.next().to_le_bytes()[..chunk.len()]);
        }
        bytes
    }
}

fn unhex(text: &str) -> Vec<u8> {
    let digit = |byte: u8| {
        u8::try_from(char::from(byte).to_digit(16).expect("hex digit")).expect("below 16")
    };
    text.as_bytes()
        .chunks(2)
        .map(|pair| digit(pair[0]) << 4 | digit(pair[1]))
        .collect()
}

#[test]
fn every_random_identifier_spells_its_own_bytes_and_parses_back() {
    let mut random = Xorshift(0x9e37_79b9_7f4a_7c15);
    for case in 0..10_000 {
        let bytes = random.bytes();
        let id = AccountId::from_random(bytes);
        assert_eq!(unhex(id.as_str()), bytes, "case {case}");
        assert_eq!(AccountId::parse(id.as_str()), Ok(id), "case {case}");
    }
}

fn pick(random: &mut Xorshift, below: usize) -> usize {
    let remainder = random
        .next()
        .checked_rem(u64::try_from(below).expect("small"))
        .expect("below is not zero");
    usize::try_from(remainder).expect("small")
}

#[test]
fn a_text_parses_exactly_when_every_one_of_its_thirty_two_characters_is_lowercase_hex() {
    const ALPHABET: &[u8; 24] = b"0123456789abcdefgABCDEF/";
    let mut random = Xorshift(0x2545_f491_4f6c_dd1d);
    let (mut accepted, mut refused) = (0_u32, 0_u32);
    for case in 0..10_000 {
        // Every case starts as 32 hex digits; odd cases then replace one position with any alphabet character,
        // so the refusals are near misses (one stray character) rather than texts that are wrong everywhere.
        let mut text: Vec<u8> = (0..32).map(|_| ALPHABET[pick(&mut random, 16)]).collect();
        if case % 2 == 1 {
            let position = pick(&mut random, text.len());
            text[position] = ALPHABET[pick(&mut random, ALPHABET.len())];
        }
        let text = String::from_utf8(text).expect("ascii");
        let expected = text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        assert_eq!(
            CompanyId::parse(&text).is_ok(),
            expected,
            "case {case}: {text:?}"
        );
        if expected {
            accepted += 1
        } else {
            refused += 1
        }
    }
    assert!(
        accepted > 5_000 && refused > 1_000,
        "accepted {accepted}, refused {refused}"
    );
}
