//! The words a company's structure is written in: roles, states and kinds of group.
//!
//! Each is stored as its word and read back through [`std::str::FromStr`], so a stored value nobody wrote (a typo,
//! a word from a later version) is refused when it is read rather than treated as some default.

use std::fmt;
use std::str::FromStr;

/// A word that is not one of the values it was read as.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{word:?} is not a {kind}")]
pub struct WordError {
    /// What it was read as.
    pub kind: &'static str,
    /// What was there.
    pub word: String,
}

macro_rules! words {
    ($(#[$doc:meta])* $name:ident, $kind:literal, { $($(#[$vdoc:meta])* $variant:ident => $word:literal),+ $(,)? }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vdoc])* $variant),+
        }

        impl $name {
            /// The word it is stored and sent as.
            #[must_use]
            pub const fn word(self) -> &'static str {
                match self {
                    $(Self::$variant => $word),+
                }
            }
        }

        impl FromStr for $name {
            type Err = WordError;

            fn from_str(word: &str) -> Result<Self, Self::Err> {
                match word {
                    $($word => Ok(Self::$variant),)+
                    _ => Err(WordError { kind: $kind, word: word.to_owned() }),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.word())
            }
        }
    };
}

words!(
    /// What a member may do in their company. A role belongs to the membership, never to a group.
    Role, "role", {
        /// Holds the company's recovery key; the only role that may make or unmake an owner.
        Owner => "owner",
        /// Manages members, departments and teams.
        Admin => "admin",
        /// Reads and writes records shared with them.
        Member => "member",
    }
);

words!(
    /// Whether a membership still counts.
    MemberState, "member state", {
        /// Counts.
        Active => "active",
        /// Kept, and refused at the next request.
        Suspended => "suspended",
    }
);

words!(
    /// Whether a company is open to its members.
    CompanyState, "company state", {
        /// Open.
        Active => "active",
        /// Closed to every member until a platform admin resumes it.
        Suspended => "suspended",
        /// Read-only, waiting to be deleted; an owner may cancel.
        Dropping => "dropping",
    }
);

words!(
    /// Whether an account may sign in at all.
    AccountState, "account state", {
        /// May sign in.
        Active => "active",
        /// Refused everywhere.
        Suspended => "suspended",
    }
);

words!(
    /// The three kinds of group. The company's own group is the root; departments sit in it and teams in a
    /// department or directly in the company.
    GroupKind, "group kind", {
        /// The company itself.
        Company => "company",
        /// A department of the company.
        Department => "department",
        /// A team, in a department or in the company.
        Team => "team",
    }
);

#[cfg(test)]
#[path = "standing_tests.rs"]
mod tests;
