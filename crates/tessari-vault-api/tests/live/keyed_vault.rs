//! A company's records vault is opened by a key the server presents with each request and nothing else: a write and
//! a read carrying the key round-trip, and the same read carrying another key, or none, is refused.

use tessari_vault_infrastructure::tessaridb::{
    Answer, Database, Place, StoreConfig, Value, VaultKey,
};

use crate::world::{Node, STORE_PASSWORD, STORE_USER};

const VAULT: &str = "records_0123456789abcdef0123456789abcdef";
const PLANTED: &str = "keyed-body-canary-5e1d";

async fn keyed_store(node: &Node, key: &VaultKey) -> (Database, Place) {
    let database = Database::new(
        StoreConfig {
            address: node.wire.clone(),
            user: STORE_USER.to_owned(),
            password: STORE_PASSWORD.to_owned(),
            authority: None,
        },
        2,
    )
    .expect("a loopback pool");
    let place = Place::new("vault_keyed", "main").expect("plain names");
    database
        .run_unplaced(
            "DEFINE NAMESPACE vault_keyed; USE NAMESPACE vault_keyed; DEFINE DATABASE main;",
            Vec::new(),
        )
        .await
        .expect("the place is defined");
    database
        .declare_keyed_vault(&place, VAULT, key)
        .await
        .expect("the vault is declared in key custody");
    database
        .run_at(
            &place,
            &format!("DEFINE FIELD body ON {VAULT} TYPE string SECRET;"),
            Vec::new(),
        )
        .await
        .expect("the sealed field is declared without the key");
    database
        .run_presenting(
            &place,
            &format!("CREATE {VAULT}:$id = {{ body: $body }};"),
            vec![
                ("id".to_owned(), Value::from("r1.1")),
                ("body".to_owned(), Value::from(PLANTED)),
            ],
            VAULT,
            key.clone(),
        )
        .await
        .expect("a write presenting the key commits");
    (database, place)
}

fn reveal() -> String {
    format!("REVEAL body FROM {VAULT}:$id;")
}

fn id() -> Vec<(String, Value)> {
    vec![("id".to_owned(), Value::from("r1.1"))]
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_keyed_write_and_reveal_round_trip_and_another_key_or_none_is_refused() {
    let node = Node::start().await;
    let key = VaultKey::new([0x5a; 32]);
    let (database, place) = keyed_store(&node, &key).await;

    let opened = database
        .run_presenting(&place, &reveal(), id(), VAULT, key.clone())
        .await
        .expect("the read presenting the key is served");
    let Some(Answer::Value {
        value: Value::Object(fields),
        ..
    }) = opened.last()
    else {
        panic!("expected the revealed fields, got {opened:?}");
    };
    assert_eq!(fields.get("body"), Some(&Value::from(PLANTED)));

    let wrong = database
        .run_presenting(&place, &reveal(), id(), VAULT, VaultKey::new([0x6b; 32]))
        .await
        .expect_err("another key opened the vault");
    let unkeyed = database
        .run_at(&place, &reveal(), id())
        .await
        .expect_err("the vault opened with no key presented");
    for (case, refused, says) in [
        ("wrong key", &wrong, "the key is wrong"),
        ("no key", &unkeyed, "the vault is sealed"),
    ] {
        let text = refused.to_string();
        assert!(
            text.contains(says),
            "{case}: refused otherwise: {refused:?}"
        );
        assert!(
            !text.contains(PLANTED),
            "{case}: the refusal carries the secret"
        );
    }
}
