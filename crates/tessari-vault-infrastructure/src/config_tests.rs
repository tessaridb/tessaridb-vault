use std::collections::BTreeMap;

use super::{NodeConfig, OidcConfig, PasskeyParty};
use crate::Error;

fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: BTreeMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |key| map.get(key).cloned()
}

const STORE: [(&str, &str); 4] = [
    (
        "TESSARIDB_VAULT_SERVICE_KEY_FILE",
        "/etc/tessaridb-vault/service.key",
    ),
    ("TESSARIDB_VAULT_STORE", "127.0.0.1:9080"),
    ("TESSARIDB_VAULT_STORE_USER", "vault"),
    ("TESSARIDB_VAULT_STORE_PASSWORD", "secret"),
];

#[test]
fn the_store_and_the_service_key_are_enough_and_the_rest_defaults() {
    let config = NodeConfig::from_lookup(lookup(&STORE)).expect("valid");
    assert_eq!(config.namespace, "vault");
    assert_eq!(config.database, "main");
    assert_eq!(config.listen.to_string(), "127.0.0.1:8080");
    assert_eq!(config.platform_admin, None);
    assert_eq!(
        config.service_key_file.to_str(),
        Some("/etc/tessaridb-vault/service.key")
    );
    assert!(
        !format!("{config:?}").contains("secret"),
        "the password never prints"
    );
}

#[test]
fn a_missing_store_setting_is_named() {
    let error = NodeConfig::from_lookup(lookup(&STORE[..3])).expect_err("no password");
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

#[test]
fn the_service_key_file_is_required_and_named() {
    let error = NodeConfig::from_lookup(lookup(&STORE[1..])).expect_err("no service key");
    assert!(matches!(
        error,
        Error::InvalidConfig {
            field: "TESSARIDB_VAULT_SERVICE_KEY_FILE",
            reason: "required",
        }
    ));
}

#[test]
fn the_oidc_issuer_is_all_three_settings_or_none() {
    let config = NodeConfig::from_lookup(lookup(&STORE)).expect("valid");
    assert_eq!(config.oidc, None);
    let mut pairs = STORE.to_vec();
    pairs.push(("TESSARIDB_VAULT_OIDC_ISSUER", "https://id.example.com"));
    pairs.push(("TESSARIDB_VAULT_OIDC_CLIENT_ID", "vault"));
    let error = NodeConfig::from_lookup(lookup(&pairs)).expect_err("two of three");
    assert!(matches!(
        error,
        Error::InvalidConfig {
            field: "TESSARIDB_VAULT_OIDC_ISSUER",
            ..
        }
    ));
    pairs.push((
        "TESSARIDB_VAULT_OIDC_REDIRECT",
        "https://vault.example.com/sign-in/return",
    ));
    let config = NodeConfig::from_lookup(lookup(&pairs)).expect("all three");
    assert_eq!(
        config.oidc,
        Some(OidcConfig {
            issuer: "https://id.example.com".to_owned(),
            client_id: "vault".to_owned(),
            redirect: "https://vault.example.com/sign-in/return".to_owned(),
        })
    );
}

#[test]
fn the_public_url_names_the_passkey_relying_party_and_must_be_https_or_loopback() {
    let config = NodeConfig::from_lookup(lookup(&STORE)).expect("valid");
    assert_eq!(config.passkeys, None);
    for (url, rp_id, origin) in [
        (
            "https://vault.example.com",
            "vault.example.com",
            "https://vault.example.com",
        ),
        (
            "https://vault.example.com:8443/",
            "vault.example.com",
            "https://vault.example.com:8443",
        ),
        (
            "http://127.0.0.1:47999",
            "127.0.0.1",
            "http://127.0.0.1:47999",
        ),
        // A browser refuses an IP address as a relying-party id, so plain-http development reaches passkeys only
        // through `localhost`, which browsers treat as a secure origin.
        (
            "http://localhost:47999",
            "localhost",
            "http://localhost:47999",
        ),
    ] {
        let mut pairs = STORE.to_vec();
        pairs.push(("TESSARIDB_VAULT_PUBLIC_URL", url));
        let config = NodeConfig::from_lookup(lookup(&pairs)).expect(url);
        assert_eq!(
            config.passkeys,
            Some(PasskeyParty {
                rp_id: rp_id.to_owned(),
                origin: origin.to_owned(),
            }),
            "{url}"
        );
    }
    for url in [
        "http://vault.example.com",
        "http://localhost.example.com",
        "http://notlocalhost:47999",
        "https://vault.example.com/vault",
        "https://user@vault.example.com",
        "not a url",
    ] {
        let mut pairs = STORE.to_vec();
        pairs.push(("TESSARIDB_VAULT_PUBLIC_URL", url));
        let error = NodeConfig::from_lookup(lookup(&pairs)).expect_err(url);
        assert!(
            matches!(
                error,
                Error::InvalidConfig {
                    field: "TESSARIDB_VAULT_PUBLIC_URL",
                    ..
                }
            ),
            "{url}"
        );
    }
}

#[test]
fn s3_attachment_storage_is_optional_but_all_four_settings_are_required_together() {
    let config = NodeConfig::from_lookup(lookup(&STORE)).expect("no S3 by default");
    assert!(config.s3.is_none());
    let required = [
        "TESSARIDB_VAULT_S3_ENDPOINT",
        "TESSARIDB_VAULT_S3_BUCKET",
        "TESSARIDB_VAULT_S3_ACCESS_KEY",
        "TESSARIDB_VAULT_S3_SECRET_KEY",
    ];
    let values = [
        "https://s3.example.com",
        "vault-attachments",
        "vault-application",
        "an-application-secret",
    ];
    for missing in required {
        let pairs: Vec<(&str, &str)> = STORE
            .iter()
            .copied()
            .chain(
                required.into_iter().zip(values).filter(|(key, _)| *key != missing),
            )
            .collect();
        let error = NodeConfig::from_lookup(lookup(&pairs)).expect_err(missing);
        assert!(
            matches!(error, Error::InvalidConfig { field, reason: "required" } if field == missing),
            "{error}"
        );
    }
    let pairs: Vec<(&str, &str)> = STORE
        .iter()
        .copied()
        .chain(required.into_iter().zip(values))
        .collect();
    let config = NodeConfig::from_lookup(lookup(&pairs)).expect("a complete S3 config");
    let s3 = config.s3.as_ref().expect("S3 is configured");
    assert_eq!(s3.endpoint, "https://s3.example.com");
    assert_eq!(s3.bucket, "vault-attachments");
    assert_eq!(s3.region, "us-east-1");
    assert!(!format!("{config:?}").contains("an-application-secret"));
}

#[test]
fn s3_requires_https_outside_a_loopback_trial() {
    let pairs: Vec<(&str, &str)> = STORE
        .iter()
        .copied()
        .chain([
            ("TESSARIDB_VAULT_S3_ENDPOINT", "http://s3.example.com"),
            ("TESSARIDB_VAULT_S3_BUCKET", "vault-attachments"),
            ("TESSARIDB_VAULT_S3_ACCESS_KEY", "vault-application"),
            ("TESSARIDB_VAULT_S3_SECRET_KEY", "an-application-secret"),
        ])
        .collect();
    let error = NodeConfig::from_lookup(lookup(&pairs)).expect_err("plain HTTP is not a trial");
    assert!(matches!(
        error,
        Error::InvalidConfig {
            field: "TESSARIDB_VAULT_S3_ENDPOINT",
            ..
        }
    ));
    let pairs: Vec<(&str, &str)> = STORE
        .iter()
        .copied()
        .chain([
            ("TESSARIDB_VAULT_S3_ENDPOINT", "http://127.0.0.1:9100"),
            ("TESSARIDB_VAULT_S3_BUCKET", "vault-attachments"),
            ("TESSARIDB_VAULT_S3_ACCESS_KEY", "vault-application"),
            ("TESSARIDB_VAULT_S3_SECRET_KEY", "an-application-secret"),
            ("TESSARIDB_VAULT_S3_REGION", "test-region-1"),
        ])
        .collect();
    let config = NodeConfig::from_lookup(lookup(&pairs)).expect("loopback HTTP is a trial");
    assert_eq!(config.s3.expect("S3").region, "test-region-1");
}
