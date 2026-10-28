//! The rules beyond "who may call this route": owner-only powers, the last owner, suspension, enumeration, groups.

use axum::http::{Method, StatusCode};
use serde_json::json;

use crate::world::{Cast, World, add_member};

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn only_an_owner_makes_or_unmakes_an_owner() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let members = format!("/api/companies/{a}/members");
    let body = json!({ "email": "second@alpha.test", "name": "Second", "role": "owner" });
    let (status, _) = world
        .call(
            Method::POST,
            &members,
            Some(&cast.admin_a),
            Some(body.clone()),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin cannot add an owner"
    );
    let promote = json!({ "role": "owner" });
    let target = format!("{members}/{}", cast.member_a_id);
    let (status, _) = world
        .call(
            Method::PATCH,
            &target,
            Some(&cast.admin_a),
            Some(promote.clone()),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin cannot promote to owner"
    );
    let (status, _) = world
        .call(
            Method::DELETE,
            &format!("{members}/{}", cast.owner_a_id),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "an admin cannot remove an owner"
    );
    let (status, _) = world
        .call(Method::PATCH, &target, Some(&cast.owner_a), Some(promote))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "an owner can");
    let (status, _) = world
        .call(
            Method::PATCH,
            &target,
            Some(&cast.admin_a),
            Some(json!({ "role": "member" })),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "and an admin cannot unmake the owner it became"
    );
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn a_company_keeps_its_last_active_owner() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let owner = format!(
        "/api/companies/{}/members/{}",
        cast.company_a, cast.owner_a_id
    );
    for change in [json!({ "role": "admin" }), json!({ "state": "suspended" })] {
        let (status, body) = world
            .call(Method::PATCH, &owner, Some(&cast.owner_a), Some(change))
            .await;
        assert_eq!(
            (status, body["error_code"].as_str()),
            (StatusCode::CONFLICT, Some("last_owner"))
        );
    }
    let (status, body) = world
        .call(Method::DELETE, &owner, Some(&cast.owner_a), None)
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("last_owner"))
    );
    // With a second owner the first may step down.
    let second = add_member(
        &world,
        &cast.company_a,
        &cast.owner_a,
        "second@alpha.test",
        "owner",
    )
    .await;
    assert_eq!(second.len(), 32);
    let (status, _) = world
        .call(
            Method::PATCH,
            &owner,
            Some(&cast.owner_a),
            Some(json!({ "role": "admin" })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn suspension_takes_effect_at_the_next_request() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let groups = format!("/api/companies/{a}/groups");
    let (status, _) = world
        .call(Method::GET, &groups, Some(&cast.admin_a), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = world
        .call(
            Method::PATCH,
            &format!("/api/companies/{a}/members/{}", cast.admin_a_id),
            Some(&cast.owner_a),
            Some(json!({ "state": "suspended" })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = world
        .call(Method::GET, &groups, Some(&cast.admin_a), None)
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a suspended member sees the company no more"
    );
    let (status, _) = world
        .call(
            Method::POST,
            &format!("/api/companies/{a}/suspend"),
            Some(&cast.platform),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = world
        .call(Method::GET, &groups, Some(&cast.owner_a), None)
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a suspended company is closed to its owner too"
    );
    let (_, body) = world
        .call(Method::GET, "/api/companies", Some(&cast.owner_a), None)
        .await;
    assert_eq!(
        body["companies"].as_array().map(Vec::len),
        Some(0),
        "{body}"
    );
    let (status, _) = world
        .call(
            Method::POST,
            &format!("/api/companies/{a}/resume"),
            Some(&cast.platform),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = world
        .call(Method::GET, &groups, Some(&cast.owner_a), None)
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn another_company_answers_exactly_as_no_company_does() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let absent = "0123456789abcdef0123456789abcdef";
    for path in ["members", "groups"] {
        let hidden = world
            .call(
                Method::GET,
                &format!("/api/companies/{}/{path}", cast.company_b),
                Some(&cast.member_a),
                None,
            )
            .await;
        let missing = world
            .call(
                Method::GET,
                &format!("/api/companies/{absent}/{path}"),
                Some(&cast.member_a),
                None,
            )
            .await;
        assert_eq!(hidden, missing);
        assert_eq!(hidden.0, StatusCode::NOT_FOUND);
    }
    // Another company's member, named inside a company the caller manages, is not found there.
    let (status, _) = world
        .call(
            Method::PATCH,
            &format!(
                "/api/companies/{}/members/{}",
                cast.company_a, cast.admin_b_id
            ),
            Some(&cast.owner_a),
            Some(json!({ "role": "admin" })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn groups_hold_their_shape_and_only_the_company_members() {
    let world = World::new().await;
    let cast = Cast::new(&world).await;
    let a = &cast.company_a;
    let groups = format!("/api/companies/{a}/groups");
    let (status, body) = world
        .call(
            Method::POST,
            &groups,
            Some(&cast.admin_a),
            Some(json!({ "kind": "department", "name": "X", "parent": cast.department_a })),
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("wrong_parent"))
    );
    let (status, body) = world
        .call(
            Method::POST,
            &groups,
            Some(&cast.admin_a),
            Some(json!({ "kind": "team", "name": "X", "parent": cast.team_a })),
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("wrong_parent")),
        "a team sits in no team"
    );
    let (status, body) = world
        .call(
            Method::DELETE,
            &format!("{groups}/{}", cast.department_a),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("group_not_empty"))
    );
    let team_members = format!("{groups}/{}/members", cast.team_a);
    let (status, _) = world
        .call(
            Method::POST,
            &team_members,
            Some(&cast.admin_a),
            Some(json!({ "account_id": cast.member_a_id })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = world
        .call(Method::GET, &team_members, Some(&cast.member_a), None)
        .await;
    assert_eq!(body["members"][0]["email"], "member@alpha.test", "{body}");
    // An account of another company cannot be put into this one's team.
    let (status, body) = world
        .call(
            Method::POST,
            &team_members,
            Some(&cast.admin_a),
            Some(json!({ "account_id": cast.admin_b_id })),
        )
        .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::CONFLICT, Some("not_a_member"))
    );
    // Removing the member from the company takes them out of its groups.
    let (status, _) = world
        .call(
            Method::DELETE,
            &format!("/api/companies/{a}/members/{}", cast.member_a_id),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = world
        .call(Method::GET, &team_members, Some(&cast.admin_a), None)
        .await;
    assert_eq!(body["members"].as_array().map(Vec::len), Some(0), "{body}");
    let (status, _) = world
        .call(
            Method::DELETE,
            &format!("{groups}/{}", cast.team_a),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = world
        .call(
            Method::DELETE,
            &format!("{groups}/{}", cast.department_a),
            Some(&cast.admin_a),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "empty now");
}
