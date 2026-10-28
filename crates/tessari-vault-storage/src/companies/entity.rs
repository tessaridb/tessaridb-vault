//! The company group row, the membership row, and what they read as.

use tessari_vault_infrastructure::tessaridb::{FromRecord, MappingFault, Row};
use tessari_vault_types::{AccountId, CompanyId, CompanyState, MemberState, Name, Role};

use crate::accounts::Account;
use crate::{Error, Result};

fn fault(record: &'static str, what: &str) -> Error {
    Error::Unreadable {
        record,
        reason: format!("{what} did not read back as written"),
    }
}

/// One `groups` row of kind `company`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompanyEntity {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) state: String,
}

impl FromRecord for CompanyEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            id: row.take("group_id")?,
            name: row.take("name")?,
            state: row.take("state")?,
        })
    }
}

/// A company.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Company {
    /// Its id, which is also its own group's.
    pub id: CompanyId,
    /// Its name.
    pub name: Name,
    /// Whether it is open to its members.
    pub state: CompanyState,
}

impl CompanyEntity {
    pub(crate) fn into_model(self) -> Result<Company> {
        Ok(Company {
            id: CompanyId::parse(&self.id).map_err(|_| fault("company", "id"))?,
            name: Name::parse(&self.name).map_err(|_| fault("company", "name"))?,
            state: self.state.parse().map_err(|_| fault("company", "state"))?,
        })
    }
}

/// One `memberships` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MembershipEntity {
    pub(crate) company: String,
    pub(crate) account: String,
    pub(crate) role: String,
    pub(crate) state: String,
}

impl FromRecord for MembershipEntity {
    fn from_row(mut row: Row) -> std::result::Result<Self, MappingFault> {
        Ok(Self {
            company: row.take("company")?,
            account: row.take("account")?,
            role: row.take("member_role")?,
            state: row.take("state")?,
        })
    }
}

/// A membership as the domain reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Membership {
    pub(crate) company: CompanyId,
    pub(crate) account: AccountId,
    pub(crate) role: Role,
    pub(crate) state: MemberState,
}

impl MembershipEntity {
    pub(crate) fn into_model(self) -> Result<Membership> {
        Ok(Membership {
            company: CompanyId::parse(&self.company).map_err(|_| fault("membership", "company"))?,
            account: AccountId::parse(&self.account).map_err(|_| fault("membership", "account"))?,
            role: self.role.parse().map_err(|_| fault("membership", "role"))?,
            state: self
                .state
                .parse()
                .map_err(|_| fault("membership", "state"))?,
        })
    }
}

/// A member of a company: their account, role and state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The member's account.
    pub account: Account,
    /// Their role in the company.
    pub role: Role,
    /// Whether their membership counts.
    pub state: MemberState,
}

/// A company as one caller sees it: the company and the caller's role there (`None` for a platform admin who is
/// not a member).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanyView {
    /// The company.
    pub company: Company,
    /// The caller's role in it.
    pub role: Option<Role>,
}
