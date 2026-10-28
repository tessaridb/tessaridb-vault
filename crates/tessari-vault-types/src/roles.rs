//! A set of roles: which members may create records of a type.

use crate::standing::Role;

/// Why a list of words is not a set of roles.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a non-empty list of the roles owner, admin and member")]
pub struct RolesError;

/// One or more of owner, admin and member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Roles {
    owner: bool,
    admin: bool,
    member: bool,
}

impl Roles {
    /// Every role.
    pub const ALL: Self = Self {
        owner: true,
        admin: true,
        member: true,
    };

    /// The roles that manage the company: owners and admins.
    pub const MANAGERS: Self = Self {
        owner: true,
        admin: true,
        member: false,
    };

    /// The set `words` names.
    ///
    /// # Errors
    ///
    /// [`RolesError`] for an empty list or a word that is not a role.
    pub fn parse<'a>(words: impl IntoIterator<Item = &'a str>) -> Result<Self, RolesError> {
        let mut set = Self {
            owner: false,
            admin: false,
            member: false,
        };
        for word in words {
            match word.parse::<Role>().map_err(|_| RolesError)? {
                Role::Owner => set.owner = true,
                Role::Admin => set.admin = true,
                Role::Member => set.member = true,
            }
        }
        if set.words().is_empty() {
            Err(RolesError)
        } else {
            Ok(set)
        }
    }

    /// Whether `role` is in the set.
    #[must_use]
    pub const fn contains(self, role: Role) -> bool {
        match role {
            Role::Owner => self.owner,
            Role::Admin => self.admin,
            Role::Member => self.member,
        }
    }

    /// The set's roles as words, owner first.
    #[must_use]
    pub fn words(self) -> Vec<&'static str> {
        [Role::Owner, Role::Admin, Role::Member]
            .into_iter()
            .filter(|role| self.contains(*role))
            .map(Role::word)
            .collect()
    }
}

#[cfg(test)]
#[path = "roles_tests.rs"]
mod tests;
