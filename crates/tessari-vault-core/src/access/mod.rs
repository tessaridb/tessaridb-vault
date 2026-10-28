//! Who may do what inside a company: one pure function over the caller's standing and the action asked for.
//!
//! The standing is read by storage in the same transaction as the work it permits, and storage calls [`decide`]
//! before it writes anything; nothing else in the server decides (authorization worksheet §12). There are no
//! prohibitions to combine: every rule is a permit, and anything no rule permits is refused.

mod decide;

pub use decide::{Action, Decision, Refusal, Standing, decide};
