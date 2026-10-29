//! Departments and teams on TessariDB: `groups` rows, their `part_of` edge, and members' `in_group` edges.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{Database, Place, Value};
use tessari_vault_types::{AccountId, CompanyId, GroupId, Name};

use super::entity::{GroupEntity, MemberRow};
use super::repository::{GroupRepository, Leaving, NewGroup};
use crate::access::{GUARD, bind, membership_id, thrown};
use crate::audit::RECORD;
use crate::records::RecordVaults;
use crate::{Result, answers};
use tessari_vault_constants::RECORD_LIST_MAX;

/// Refuses unless `$group` is a department or team of `$company`, the answer a group of another company gets.
const GROUP_OF_COMPANY: &str = "\
THROW 'not found' IF (GET groups:$group).company != $company OR (GET groups:$group).kind = 'company';";

pub(crate) struct TessaridbGroupRepository {
    database: Database,
}

impl TessaridbGroupRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }

    async fn write(
        &self,
        place: &Place,
        script: &str,
        parameters: Vec<(String, Value)>,
    ) -> Result<()> {
        self.database
            .run_at(place, script, parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }
}

#[async_trait]
impl GroupRepository for TessaridbGroupRepository {
    async fn list(&self, place: &Place, company: &CompanyId) -> Result<Vec<GroupEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM groups WHERE company = $company AND kind != 'company' USING INDEX by_company;",
                vec![bind("company", company.as_str())],
            )
            .await?;
        answers::records(answers, "group")
    }

    async fn get(
        &self,
        place: &Place,
        company: &CompanyId,
        group: &GroupId,
    ) -> Result<Option<GroupEntity>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT * FROM groups WHERE group_id = $group AND company = $company AND kind != 'company' \
                 USING INDEX by_id;",
                vec![bind("group", group.as_str()), bind("company", company.as_str())],
            )
            .await?;
        answers::single(answers, "group")
    }

    async fn members(&self, place: &Place, group: &GroupId) -> Result<Vec<MemberRow>> {
        let answers = self
            .database
            .run_at(
                place,
                "SELECT account_id FROM groups:$group<-in_group<-accounts;",
                vec![bind("group", group.as_str())],
            )
            .await?;
        answers::records(answers, "group member")
    }

    async fn create(
        &self,
        place: &Place,
        mut parameters: Vec<(String, Value)>,
        company: &CompanyId,
        group: &NewGroup,
    ) -> Result<()> {
        let parent = &group.parent;
        parameters.extend([
            bind("company", company.as_str()),
            bind("group", group.id.as_str()),
            bind("kind", group.kind.word()),
            bind("name", group.name.as_str()),
            bind("parent", parent.as_str()),
        ]);
        // A department sits in the company; a team in the company or in one of its departments.
        let parent_check = if parent.as_str() == company.as_str() {
            ""
        } else {
            "THROW 'wrong parent' IF (GET groups:$parent).company != $company \
             OR (GET groups:$parent).kind != 'department';"
        };
        let script = format!(
            "BEGIN; {GUARD} {parent_check} \
             CREATE groups:$group = {{ group_id: $group, company: $company, kind: $kind, name: $name, \
             parent: $parent, state: 'active', created: time::now() }}; \
             RELATE groups:$group->part_of->groups:$parent; {RECORD} COMMIT;"
        );
        self.write(place, &script, parameters).await
    }

    async fn rename(
        &self,
        place: &Place,
        mut parameters: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        name: &Name,
    ) -> Result<()> {
        parameters.extend([
            bind("company", company.as_str()),
            bind("group", group.as_str()),
            bind("name", name.as_str()),
        ]);
        let script = format!(
            "BEGIN; {GUARD} {GROUP_OF_COMPANY} UPDATE groups:$group SET name = $name; {RECORD} COMMIT;"
        );
        self.write(place, &script, parameters).await
    }

    async fn delete(
        &self,
        place: &Place,
        mut parameters: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupEntity,
        members: &[AccountId],
    ) -> Result<()> {
        parameters.extend([
            bind("company", company.as_str()),
            bind("group", group.id.as_str()),
            bind("parent", group.parent.as_str()),
        ]);
        let mut edges = String::new();
        for (index, account) in members.iter().enumerate() {
            let name = format!("a{index}");
            edges.push_str(&format!(
                "DELETE accounts:${name}->in_group->groups:$group; "
            ));
            parameters.push(bind(&name, account.as_str()));
        }
        let script = format!(
            "BEGIN; {GUARD} {GROUP_OF_COMPANY} \
             LET $inside = SELECT group_id FROM groups:$group<-part_of<-groups LIMIT 1; \
             THROW 'group not empty' IF array::len($inside) > 0; \
             {edges} DELETE groups:$group->part_of->groups:$parent; DELETE groups:$group; {RECORD} COMMIT;"
        );
        self.write(place, &script, parameters).await
    }

    async fn add_member(
        &self,
        place: &Place,
        mut parameters: Vec<(String, Value)>,
        company: &CompanyId,
        group: &GroupId,
        account: &AccountId,
    ) -> Result<()> {
        parameters.extend([
            bind("company", company.as_str()),
            bind("group", group.as_str()),
            bind("account", account.as_str()),
            bind("membership", membership_id(company, account).as_str()),
        ]);
        let script = format!(
            "BEGIN; {GUARD} {GROUP_OF_COMPANY} \
             THROW 'not a member' IF (GET memberships:$membership).state != 'active'; \
             RELATE accounts:$account->in_group->groups:$group; {RECORD} COMMIT;"
        );
        self.write(place, &script, parameters).await
    }

    async fn retiring(
        &self,
        place: &Place,
        company: &CompanyId,
        group: &GroupId,
    ) -> Result<Option<Vec<String>>> {
        let Some(entity) = self.get(place, company, group).await? else {
            return Ok(None);
        };
        let mut affected = vec![entity.id.clone()];
        if let Ok(parent) = GroupId::parse(&entity.parent)
            && let Some(department) = self.get(place, company, &parent).await?
            && department.kind == "department"
        {
            affected.push(department.id);
        }
        Ok(Some(affected))
    }

    async fn remove_member(
        &self,
        place: &Place,
        mut parameters: Vec<(String, Value)>,
        leaving: Leaving<'_>,
    ) -> Result<()> {
        let vault = RecordVaults::name(leaving.company);
        parameters.extend([
            bind("company", leaving.company.as_str()),
            bind("group", leaving.group.as_str()),
            bind("account", leaving.account.as_str()),
            bind("gvid", format!("group.{}", leaving.group.as_str()).as_str()),
            bind(
                "affected",
                Value::Array(leaving.affected.into_iter().map(Value::from).collect()),
            ),
            bind("limit", RECORD_LIST_MAX),
        ]);
        let revoke = match &leaving.revocation {
            Some(revocation) => {
                parameters.push(bind("revoked", revocation.recipient.as_str()));
                format!(" REMOVE RECIPIENT $revoked FROM {vault}:$gvid;")
            }
            None => String::new(),
        };
        // The keys the leaver could hold retire, and so does every record shared through them; the version moves
        // so a read decided before this commit refuses.
        let script = format!(
            "BEGIN; {GUARD} {GROUP_OF_COMPANY} DELETE accounts:$account->in_group->groups:$group; \
             UPDATE group_keys SET rotate = true WHERE company = $company AND group_id IN $affected; \
             UPDATE records SET rotate = true, share_version = share_version + 1 WHERE company = $company \
             AND record_id IN (SELECT record FROM shares WHERE company = $company AND target IN $affected \
             LIMIT $limit);{revoke} {RECORD} COMMIT;"
        );
        match leaving.revocation {
            Some(revocation) => {
                self.database
                    .run_presenting(place, &script, parameters, &vault, revocation.vault_key)
                    .await
                    .map_err(|error| thrown(error.into()))?;
                Ok(())
            }
            None => self.write(place, &script, parameters).await,
        }
    }
}
