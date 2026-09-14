//! Rust access to the Lumen database.
//!
//! The SQLAlchemy models in `backend/lumen/db/models.py` are the single source
//! of truth for the schema. `generated.rs` is produced from them by
//! `scripts/generate_models.py`, and a pytest fails when the two drift — a Rust
//! schema maintained by hand alongside 447 Alembic revisions would diverge, and
//! a diverged schema corrupts data quietly.
//!
//! The crate carries the row types, a connection pool, and per-tenant schema
//! scoping. Shard routing is refused rather than approximated — see
//! [`connection::ShardError`].

pub mod connection;
pub mod generated;

pub use connection::{Database, DatabaseConfig, TenantId};
pub use generated::*;

use serde::{Deserialize, Serialize};

/// Ciphertext read straight out of a column.
///
/// The encryption key lives in the Python backend, so Rust can move these bytes
/// but cannot read them. The newtype exists to make that impossible to forget:
/// `Sandbox.encrypted_pat` is not a token, it is a sealed blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Encrypted(pub Vec<u8>);

impl Encrypted {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A column held a value the generated enum does not know.
///
/// This means the database is ahead of the generated file — a migration landed
/// without regenerating. It is reported rather than defaulted, because guessing
/// a variant here is how a policy check silently passes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{enum_name} has no variant for {value:?}; regenerate the Rust models")]
pub struct UnknownEnumValue {
    pub enum_name: &'static str,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn enum_values_round_trip_through_their_database_text() {
        let status = SandboxStatus::from_str(SandboxStatus::Running.as_str())
            .expect("the value it just emitted parses back");
        assert_eq!(status, SandboxStatus::Running);
    }

    #[test]
    fn an_unknown_enum_value_is_reported_not_defaulted() {
        let error = SandboxStatus::from_str("teleporting").expect_err("unknown value");
        assert_eq!(error.enum_name, "SandboxStatus");
        assert_eq!(error.value, "teleporting");
        assert!(error.to_string().contains("regenerate the Rust models"));
    }

    #[test]
    fn every_model_names_its_table_and_columns() {
        assert_eq!(Sandbox::TABLE, "sandbox");
        assert!(Sandbox::COLUMNS.contains(&"encrypted_pat"));
        assert!(Sandbox::COLUMNS.contains(&"id"));
        assert!(!Sandbox::COLUMNS.is_empty());
    }

    #[test]
    fn ciphertext_is_not_a_string() {
        // The type system is the reminder: there is no `as_str` here.
        let sealed = Encrypted(vec![1, 2, 3]);
        assert_eq!(sealed.as_bytes(), &[1, 2, 3]);
        assert!(!sealed.is_empty());
    }
}
