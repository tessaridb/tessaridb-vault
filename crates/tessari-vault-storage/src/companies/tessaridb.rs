//! Companies and memberships on TessariDB: the dedicated `companies` row and the `memberships` table.
//!
//! Every write that rests on a decision starts with [`GUARD`], so the transaction is refused when the caller's
//! standing moved after the decision was taken.

use async_trait::async_trait;
use tessari_vault_infrastructure::tessaridb::{
    Database, FromRecord, MappingFault, Place, Row, Value,
};
use tessari_vault_types::{AccountId, CompanyId, CompanyState, GroupId, MemberState, Name, Role};

use super::entity::{CompanyEntity, Membership, MembershipEntity};
use super::repository::{Change, CompanyRepository, Joining};
use crate::access::{GUARD, bind, membership_id, thrown};
use crate::audit::RECORD;
use crate::shares::retiring;
use crate::{Error, Result, answers};

/// The statement that refuses a change taking away the company's last active owner. Binds `$company`.
const KEEPS_AN_OWNER: &str = "\
LET $owners = SELECT count(*) AS n FROM memberships WHERE company = companies:$company AND member_role = 'owner' \
AND is_active = true; THROW 'last owner' IF $owners[0].n <= 1;";

/// Run beside [`KEEPS_AN_OWNER`] when an owner stops being one: whatever they opened of the recovery key, it retires.
const RETIRES_RECOVERY: &str =
    "UPDATE recovery_keys SET rotate = true WHERE company = companies:$company;";

/// The statement that refuses a write when the target membership is not what was read. Binds `$target`,
/// `$target_role`, `$target_state`.
const TARGET_UNCHANGED: &str = "\
THROW 'standing changed' IF (GET memberships:$target).member_role != $target_role \
OR (GET memberships:$target).is_active != $target_active;";

pub(crate) struct TessaridbCompanyRepository {
    database: Database,
}

impl TessaridbCompanyRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

/// The account a membership is for, created in the transaction when it is new.
fn joining_statement(joining: &Joining, parameters: &mut Vec<(String, Value)>) -> &'static str {
    match joining {
        Joining::Existing(_) => "",
        Joining::New { id, email, name } => {
            parameters.push(bind("new_account", id.as_str()));
            parameters.push(bind("new_email", email.as_str()));
            parameters.push(bind("new_name", name.as_str()));
            "CREATE accounts:$new_account = { email: $new_email, name: $new_name, is_active: true, \
             platform_admin: false, created: time::now() };"
        }
    }
}

const NEW_MEMBERSHIP: &str = "\
CREATE memberships:$membership = { company: companies:$company, account: accounts:$account, member_role: $role, is_active: true, \
joined: time::now() };";

struct GroupRow {
    group: String,
    company: String,
    parent: String,
}

impl FromRecord for GroupRow {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            group: row.id().trim_matches('\'').to_owned(),
            company: crate::answers::reference(&mut row, "company")?,
            parent: crate::answers::reference(&mut row, "parent")?,
        })
    }
}

#[async_trait]
impl CompanyRepository for TessaridbCompanyRepository {
    async fn create(
        &self,
        place: &Place,
        audit: Vec<(String, Value)>,
        company: &CompanyId,
        name: &Name,
        key_epoch: u32,
        owner: &Joining,
    ) -> Result<()> {
        let mut parameters = audit;
        parameters.extend([
            bind("company", company.as_str()),
            bind("name", name.as_str()),
            bind("key_epoch", i64::from(key_epoch)),
            bind("membership", membership_id(company, owner.id()).as_str()),
            bind("account", owner.id().as_str()),
            bind("role", Role::Owner.word()),
        ]);
        let account = joining_statement(owner, &mut parameters);
        let script = format!(
            "BEGIN; {account} \
             CREATE companies:$company = {{ name: $name, is_active: true, is_suspended: false, \
             is_dropping: false, key_epoch: $key_epoch, created: time::now() }}; {NEW_MEMBERSHIP} {RECORD} COMMIT;"
        );
        self.database
            .transact_at(place, &script, "", parameters)
            .await
            .map_err(|error| match error {
                tessari_vault_infrastructure::Error::Refused { message, .. }
                    if message.starts_with("unique index by_name") =>
                {
                    Error::Conflict(crate::Conflict::CompanyExists)
                }
                other => thrown(other.into()),
            })?;
        Ok(())
    }

    async fn all(&self, place: &Place) -> Result<Vec<CompanyEntity>> {
        let answers = self
            .database
            .read_at(place, "SELECT * FROM companies;", Vec::new())
            .await?;
        answers::records(answers, "company")
    }

    async fn by_ids(
        &self,
        place: &Place,
        wanted: &[CompanyId],
        only_open: bool,
    ) -> Result<Vec<CompanyEntity>> {
        if wanted.is_empty() {
            return Ok(Vec::new());
        }
        let mut found = Vec::new();
        for company in wanted {
            let answers = self
                .database
                .read_at(
                    place,
                    "SELECT * FROM companies:$id;",
                    vec![bind("id", company.as_str())],
                )
                .await?;
            let Some(entity): Option<CompanyEntity> = answers::single(answers, "company")? else {
                continue;
            };
            if only_open && !entity.is_active {
                continue;
            }
            found.push(entity);
        }
        Ok(found)
    }

    async fn memberships_of(
        &self,
        place: &Place,
        account: &AccountId,
    ) -> Result<Vec<MembershipEntity>> {
        let answers = self
            .database
            .read_at(
                place,
                "SELECT * FROM memberships WHERE account = accounts:$account USING INDEX by_account;",
                vec![bind("account", account.as_str())],
            )
            .await?;
        answers::records(answers, "membership")
    }

    async fn set_state(
        &self,
        place: &Place,
        audit: Vec<(String, Value)>,
        company: &CompanyId,
        state: CompanyState,
    ) -> Result<()> {
        let mut parameters = audit;
        parameters.extend([
            bind("company", company.as_str()),
            bind("active", matches!(state, CompanyState::Active)),
            bind("suspended", matches!(state, CompanyState::Suspended)),
            bind("dropping", matches!(state, CompanyState::Dropping)),
        ]);
        let script = format!(
            "BEGIN; THROW 'not found' IF (GET companies:$company) = NONE; \
             UPDATE companies:$company SET is_active = $active, is_suspended = $suspended, \
             is_dropping = $dropping; {RECORD} COMMIT;"
        );
        self.database
            .transact_at(place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn members(&self, place: &Place, company: &CompanyId) -> Result<Vec<MembershipEntity>> {
        let answers = self
            .database
            .read_at(
                place,
                "SELECT * FROM memberships WHERE company = companies:$company USING INDEX by_company;",
                vec![bind("company", company.as_str())],
            )
            .await?;
        answers::records(answers, "membership")
    }

    async fn membership(
        &self,
        place: &Place,
        company: &CompanyId,
        account: &AccountId,
    ) -> Result<Option<MembershipEntity>> {
        let answers = self
            .database
            .read_at(
                place,
                "SELECT * FROM memberships:$membership;",
                vec![bind("membership", membership_id(company, account).as_str())],
            )
            .await?;
        answers::single(answers, "membership")
    }

    async fn add_member(
        &self,
        place: &Place,
        mut parameters: Vec<(String, Value)>,
        company: &CompanyId,
        joining: &Joining,
        role: Role,
    ) -> Result<()> {
        parameters.extend([
            bind("company", company.as_str()),
            bind("membership", membership_id(company, joining.id()).as_str()),
            bind("account", joining.id().as_str()),
            bind("role", role.word()),
        ]);
        let account = joining_statement(joining, &mut parameters);
        let script = format!(
            "BEGIN; {GUARD} THROW 'already a member' IF (GET memberships:$membership).company = companies:$company; \
             {account} {NEW_MEMBERSHIP} {RECORD} COMMIT;"
        );
        self.database
            .transact_at(place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn change_member(
        &self,
        place: &Place,
        mut parameters: Vec<(String, Value)>,
        target: &Membership,
        change: Change,
    ) -> Result<()> {
        parameters.extend(target_values(target));
        parameters.extend([
            bind("role", change.role.word()),
            bind("active", matches!(change.state, MemberState::Active)),
        ]);
        let owners = if change.loses_owner {
            format!("{KEEPS_AN_OWNER} {RETIRES_RECOVERY}")
        } else {
            String::new()
        };
        let script = format!(
            "BEGIN; {GUARD} {TARGET_UNCHANGED} {owners} \
             UPDATE memberships:$target SET member_role = $role, is_active = $active; {RECORD} COMMIT;"
        );
        self.database
            .transact_at(place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn remove_member(
        &self,
        place: &Place,
        mut parameters: Vec<(String, Value)>,
        target: &Membership,
        groups: &[GroupId],
        loses_owner: bool,
        retired: Vec<String>,
    ) -> Result<()> {
        parameters.extend(target_values(target));
        parameters.push(bind("account", target.account.as_str()));
        let mut edges = String::new();
        for (index, group) in groups.iter().enumerate() {
            let name = format!("g{index}");
            edges.push_str(&format!(
                "DELETE accounts:$account->in_group->groups:${name}; "
            ));
            parameters.push(bind(&name, group.as_str()));
        }
        let owners = if loses_owner {
            format!("{KEEPS_AN_OWNER} {RETIRES_RECOVERY}")
        } else {
            String::new()
        };
        // Every key the leaver could hold retires, with every record they wrote or that was shared through them.
        let retire = retiring(
            &self.database,
            place,
            &mut parameters,
            &target.company,
            retired,
            Some(&target.account),
        )
        .await?;
        let script = format!(
            "BEGIN; {GUARD} {TARGET_UNCHANGED} {owners} {edges} DELETE memberships:$target; {retire} {RECORD} COMMIT;"
        );
        self.database
            .transact_at(place, &script, "", parameters)
            .await
            .map_err(|error| thrown(error.into()))?;
        Ok(())
    }

    async fn groups_of(
        &self,
        place: &Place,
        account: &AccountId,
    ) -> Result<Vec<(String, String, String)>> {
        let answers = self
            .database
            .read_at(
                place,
                "SELECT id, company, parent FROM accounts:$account->in_group->groups;",
                vec![bind("account", account.as_str())],
            )
            .await?;
        let rows: Vec<GroupRow> = answers::records(answers, "group")?;
        Ok(rows
            .into_iter()
            .map(|row| (row.group, row.company, row.parent))
            .collect())
    }
}

fn target_values(target: &Membership) -> [(String, Value); 4] {
    [
        bind(
            "target",
            membership_id(&target.company, &target.account).as_str(),
        ),
        bind("target_role", target.role.word()),
        bind("target_active", matches!(target.state, MemberState::Active)),
        bind("company", target.company.as_str()),
    ]
}
