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

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_company_is_keyed_under_the_service_key_epoch_current_when_it_was_made() {
    use crate::world::{Cast, World};

    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let current = world.vault_keys.current_epoch();
    assert!(
        current > 1,
        "the live ring holds an older epoch beside the current one"
    );
    let row = world
        .database
        .run_at(
            &world.place,
            "RETURN (GET groups:$c).key_epoch;",
            vec![("c".to_owned(), Value::from(cast.company_a.as_str()))],
        )
        .await
        .expect("the company row reads back");
    assert_eq!(
        row.last().and_then(|answer| match answer {
            Answer::Value { value, .. } => Some(value.clone()),
            _ => None,
        }),
        Some(Value::from(i64::from(current))),
        "the company row names the epoch its vault is keyed under"
    );
    let vault = format!("records_{}", cast.company_a);
    let probe = format!("CREATE {vault}:'epoch-probe' = {{ body: 'epoch-canary' }};");
    let older = world
        .vault_keys
        .company(1, &cast.company_a)
        .expect("KV(A) under epoch 1");
    let refused = world
        .database
        .run_presenting(&world.place, &probe, Vec::new(), &vault, older)
        .await
        .expect_err("the older epoch's key opened a vault keyed under the current one");
    assert!(
        refused.to_string().contains("the key is wrong"),
        "{refused:?}"
    );
    let keyed = world
        .vault_keys
        .company(current, &cast.company_a)
        .expect("KV(A) under the current epoch");
    world
        .database
        .run_presenting(&world.place, &probe, Vec::new(), &vault, keyed)
        .await
        .expect("the current epoch's key opens the vault");
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_keyed_vault_rewrapped_opens_with_the_new_key_and_refuses_the_old() {
    let node = Node::start().await;
    let old = VaultKey::new([0x5a; 32]);
    let new = VaultKey::new([0x7c; 32]);
    let (database, place) = keyed_store(&node, &old).await;
    database
        .change_keyed_vault(&place, VAULT, &old, &new)
        .await
        .expect("the vault is re-wrapped under the new key");
    let opened = database
        .run_presenting(&place, &reveal(), id(), VAULT, new.clone())
        .await
        .expect("the new key opens the vault");
    assert!(
        matches!(opened.last(), Some(Answer::Value { value: Value::Object(fields), .. })
            if fields.get("body") == Some(&Value::from(PLANTED))),
        "{opened:?}"
    );
    let refused = database
        .run_presenting(&place, &reveal(), id(), VAULT, old.clone())
        .await
        .expect_err("the old key still opens the vault");
    assert!(
        refused.to_string().contains("the key is wrong"),
        "{refused:?}"
    );
    let again = database
        .change_keyed_vault(&place, VAULT, &old, &new)
        .await
        .expect_err("a second re-wrap from the old key was accepted");
    assert!(!again.to_string().contains(PLANTED), "{again:?}");
}
