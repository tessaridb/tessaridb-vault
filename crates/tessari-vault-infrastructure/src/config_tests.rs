use std::collections::BTreeMap;

use super::NodeConfig;
use crate::Error;

fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: BTreeMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |key| map.get(key).cloned()
}

const STORE: [(&str, &str); 3] = [
    ("TESSARIDB_VAULT_STORE", "127.0.0.1:9080"),
    ("TESSARIDB_VAULT_STORE_USER", "vault"),
    ("TESSARIDB_VAULT_STORE_PASSWORD", "secret"),
];

#[test]
fn the_store_alone_is_enough_and_the_rest_defaults() {
    let config = NodeConfig::from_lookup(lookup(&STORE)).expect("valid");
    assert_eq!(config.namespace, "vault");
    assert_eq!(config.database, "main");
    assert_eq!(config.listen.to_string(), "127.0.0.1:8080");
    assert_eq!(config.platform_admin, None);
    assert!(
        !format!("{config:?}").contains("secret"),
        "the password never prints"
    );
}

#[test]
fn a_missing_store_setting_is_named() {
    let error = NodeConfig::from_lookup(lookup(&STORE[..2])).expect_err("no password");
    assert!(matches!(
        error,
        Error::InvalidConfig {
            field: "TESSARIDB_VAULT_STORE_PASSWORD",
            ..
        }
    ));
}

#[test]
fn the_platform_admin_is_both_halves_or_neither() {
    let mut pairs = STORE.to_vec();
    pairs.push(("TESSARIDB_VAULT_PLATFORM_ADMIN_EMAIL", "ops@example.com"));
    let error = NodeConfig::from_lookup(lookup(&pairs)).expect_err("half set");
    assert!(matches!(
        error,
        Error::InvalidConfig {
            field: "TESSARIDB_VAULT_PLATFORM_ADMIN_EMAIL",
            ..
        }
    ));
    pairs.push(("TESSARIDB_VAULT_PLATFORM_ADMIN_NAME", "Ops"));
    let config = NodeConfig::from_lookup(lookup(&pairs)).expect("both set");
    assert_eq!(
        config.platform_admin,
        Some(("ops@example.com".to_owned(), "Ops".to_owned()))
    );
}
