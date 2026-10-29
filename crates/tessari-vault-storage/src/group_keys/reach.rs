//! The reach of a group's key: the members who keep a key set inside a team, a department with its teams, or the
//! whole company, read from the store and never from the request.

use std::collections::HashSet;

use tessari_vault_constants::KEY_LIST_MAX;
use tessari_vault_infrastructure::tessaridb::{
    Database, FromRecord, MappingFault, Place, Row, Value,
};
use tessari_vault_types::{CompanyId, GroupId};

use super::entity::GroupHead;
use crate::access::bind;
use crate::keys::KeyEntity;
use crate::{Error, Result, answers};

/// One projected `name`.
struct Named {
    name: String,
}

impl FromRecord for Named {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            name: row.take("name")?,
        })
    }
}

/// The `name` of every record the script's first answer holds.
async fn names(
    database: &Database,
    place: &Place,
    script: &str,
    parameters: Vec<(String, Value)>,
) -> Result<HashSet<String>> {
    let rows: Vec<Named> =
        answers::records(database.run_at(place, script, parameters).await?, "name")?;
    Ok(rows.into_iter().map(|row| row.name).collect())
}

async fn team_members(database: &Database, place: &Place, team: &str) -> Result<HashSet<String>> {
    names(
        database,
        place,
        "SELECT account_id AS name FROM groups:$group<-in_group<-accounts;",
        vec![bind("group", team)],
    )
    .await
}

pub(super) async fn reach(
    database: &Database,
    place: &Place,
    company: &CompanyId,
    group: &GroupId,
) -> Result<Option<Vec<KeyEntity>>> {
    let heads: Option<GroupHead> = answers::single(
        database
            .run_at(
                place,
                "SELECT company, kind FROM groups:$group;",
                vec![bind("group", group.as_str())],
            )
            .await?,
        "group",
    )?;
    let Some(head) = heads.filter(|head| head.company == company.as_str()) else {
        return Ok(None);
    };
    // Membership runs upward: a team's members are its department's, and every member is the company's.
    let members = names(
        database,
        place,
        "SELECT account AS name FROM memberships WHERE company = $company AND state = 'active' \
             LIMIT $limit USING INDEX by_company;",
        vec![
            bind("company", company.as_str()),
            bind("limit", KEY_LIST_MAX),
        ],
    )
    .await?;
    let reach = match head.kind.as_str() {
        "company" => members.clone(),
        "team" => team_members(database, place, group.as_str()).await?,
        "department" => {
            let mut reach = team_members(database, place, group.as_str()).await?;
            let teams = names(
                database,
                place,
                "SELECT group_id AS name FROM groups:$group<-part_of<-groups;",
                vec![bind("group", group.as_str())],
            )
            .await?;
            for team in teams {
                reach.extend(team_members(database, place, &team).await?);
            }
            reach
        }
        _ => return Err(unreadable("group kind")),
    };
    let keys: Vec<KeyEntity> = answers::records(
        database
            .run_at(
                place,
                "SELECT * FROM member_keys WHERE company = $company LIMIT $limit USING INDEX by_company;",
                vec![
                    bind("company", company.as_str()),
                    bind("limit", KEY_LIST_MAX),
                ],
            )
            .await?,
        "key set",
    )?;
    Ok(Some(
        keys.into_iter()
            .filter(|key| reach.contains(&key.account) && members.contains(&key.account))
            .collect(),
    ))
}

fn unreadable(what: &str) -> Error {
    Error::Unreadable {
        record: "group key",
        reason: format!("{what} did not read back as written"),
    }
}
