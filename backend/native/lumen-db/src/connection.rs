//! Connecting to the Lumen database, one tenant at a time.
//!
//! Port of `get_session_with_tenant` in
//! `backend/lumen/db/engine/sql_engine.py`. A tenant selects two things there:
//! the schema, through SQLAlchemy's `schema_translate_map`, and the physical
//! database, through the shard registry.
//!
//! The schema half is implemented here. The shard half is deliberately not: see
//! [`ShardError`].

use std::str::FromStr;
use std::time::Duration;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;

/// The schema every self-hosted deployment uses.
pub const DEFAULT_SCHEMA: &str = "public";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid tenant id {0:?}: a schema name must match ^[a-zA-Z0-9_-]+$")]
pub struct InvalidTenantId(pub String);

/// A validated tenant id, which is also a Postgres schema name.
///
/// The only way to build one is through [`TenantId::parse`], so a value of this
/// type has already passed the same check the Python `is_valid_schema_name`
/// applies. That matters because the id reaches a `SET search_path`: an
/// unvalidated string there is SQL injection across every tenant boundary.
///
/// The name is also quoted when it is used. The regex allows `-`, which is not
/// a bare identifier in Postgres, so quoting is needed for correctness as well
/// as safety — and having both means neither one alone is load-bearing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(String);

impl TenantId {
    /// Port of `is_valid_schema_name`: `^[a-zA-Z0-9_-]+$`.
    ///
    /// Written by hand rather than with a regex so the rule is visible at the
    /// point it is enforced.
    pub fn parse(value: impl Into<String>) -> Result<Self, InvalidTenantId> {
        let value = value.into();
        let valid = !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');

        if valid {
            Ok(Self(value))
        } else {
            Err(InvalidTenantId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_default_schema(&self) -> bool {
        self.0 == DEFAULT_SCHEMA
    }

    /// The identifier as it is written into SQL.
    ///
    /// `parse` already rejects a quote, so this cannot be escaped out of; the
    /// quoting is here because `-` is legal in a tenant id and illegal in a bare
    /// identifier.
    pub fn quoted(&self) -> String {
        format!("\"{}\"", self.0.replace('"', "\"\""))
    }
}

impl FromStr for TenantId {
    type Err = InvalidTenantId;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// Routing a tenant to a physical database is not implemented.
///
/// The Python router consults a catalog table, an in-process TTL cache and a
/// Redis version counter, and **fails closed**: if it cannot resolve a tenant it
/// raises rather than assuming the default shard, because guessing "default" for
/// an already-migrated tenant sends its writes to the database it moved off.
///
/// This crate keeps that property the only way it honestly can while the router
/// is unported: a sharded deployment is refused outright. Silently using the
/// default engine would be the exact mistake the Python code documents.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "this build routes every tenant to one database; \
     sharded deployments are not supported yet"
)]
pub struct ShardError;

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error(transparent)]
    InvalidTenantId(#[from] InvalidTenantId),
    #[error(transparent)]
    Shard(#[from] ShardError),
    #[error("database configuration is invalid: {0}")]
    Config(String),
    #[error("could not get a connection from the pool: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),
    #[error("database error: {0}")]
    Postgres(#[from] tokio_postgres::Error),
}

/// How to reach the database.
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub pool_size: usize,
    pub connect_timeout: Duration,
    /// Mirrors `MULTI_TENANT`. Kept because callers branch on it; the pool
    /// itself does not, since it scopes every borrow regardless.
    pub multi_tenant: bool,
    /// Mirrors `is_sharded()`. Any value but false is refused; see [`ShardError`].
    pub sharded: bool,
}

impl DatabaseConfig {
    /// Read the same environment the Python engine reads.
    pub fn from_env() -> Self {
        fn var(name: &str, fallback: &str) -> String {
            std::env::var(name).unwrap_or_else(|_| fallback.to_string())
        }

        Self {
            host: var("POSTGRES_HOST", "localhost"),
            port: var("POSTGRES_PORT", "5432").parse().unwrap_or(5432),
            user: var("POSTGRES_USER", "postgres"),
            password: var("POSTGRES_PASSWORD", "password"),
            database: var("POSTGRES_DB", "postgres"),
            pool_size: var("POSTGRES_POOL_SIZE", "10").parse().unwrap_or(10),
            connect_timeout: Duration::from_secs(10),
            multi_tenant: var("MULTI_TENANT", "").to_lowercase() == "true",
            sharded: !var("LUMEN_DB_SHARD_SPECS_JSON", "").trim().is_empty(),
        }
    }
}

/// A connection pool for one physical database.
///
/// It holds no `multi_tenant` flag: the scope is set on every borrow either
/// way, so there is no branch for the flag to select.
#[derive(Clone, Debug)]
pub struct Database {
    pool: Pool,
}

impl Database {
    pub fn connect(config: &DatabaseConfig) -> Result<Self, ConnectionError> {
        if config.sharded {
            return Err(ShardError.into());
        }

        let mut pg_config = tokio_postgres::Config::new();
        pg_config
            .host(&config.host)
            .port(config.port)
            .user(&config.user)
            .password(&config.password)
            .dbname(&config.database)
            .connect_timeout(config.connect_timeout);

        let manager = Manager::from_config(
            pg_config,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        );

        let pool = Pool::builder(manager)
            .max_size(config.pool_size)
            .build()
            .map_err(|err| ConnectionError::Config(err.to_string()))?;

        Ok(Self { pool })
    }

    /// Take a connection scoped to one tenant's schema.
    ///
    /// Mirrors `get_session_with_tenant`. The scope is set on **every** borrow,
    /// never skipped: SQLAlchemy carries the schema as a per-connection
    /// execution option, but `SET search_path` is session state that outlives
    /// the borrow. A pooled connection handed back with a tenant's search_path
    /// still set is a cross-tenant read for whoever takes it next, and an
    /// integration test in `tests/tenant_scoping.rs` caught exactly that.
    ///
    /// The Python shortcut for a self-hosted deployment on the default schema is
    /// therefore not reproduced as a skip. It is an optimisation there because
    /// the mechanism cannot leak; here it could.
    pub async fn tenant(
        &self,
        tenant_id: &TenantId,
    ) -> Result<deadpool_postgres::Object, ConnectionError> {
        let client = self.pool.get().await?;

        client
            .batch_execute(&format!("SET search_path TO {}", tenant_id.quoted()))
            .await?;

        Ok(client)
    }

    /// Take a connection with no tenant scope, for catalog work.
    ///
    /// `RESET` rather than `SET ... TO public`: it restores the server's own
    /// default, whatever that is, instead of guessing at it.
    pub async fn unscoped(&self) -> Result<deadpool_postgres::Object, ConnectionError> {
        let client = self.pool.get().await?;
        client.batch_execute("RESET search_path").await?;
        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tenant_id_accepts_what_the_python_regex_accepts() {
        for value in ["public", "tenant_1", "tenant-1", "ABC123", "a"] {
            assert!(TenantId::parse(value).is_ok(), "{value:?} should parse");
        }
    }

    #[test]
    fn a_tenant_id_refuses_anything_that_could_reach_sql() {
        for value in [
            "",
            "public; drop schema public cascade",
            "public schema",
            "public\"",
            "public'",
            "tenant.1",
            "tenant$1",
            "tenant\n",
            "té",
        ] {
            assert!(
                TenantId::parse(value).is_err(),
                "{value:?} must be rejected"
            );
        }
    }

    #[test]
    fn the_identifier_is_quoted_because_a_dash_is_legal_in_a_tenant_id() {
        let tenant = TenantId::parse("tenant-1").expect("parses");
        assert_eq!(tenant.quoted(), "\"tenant-1\"");
    }

    #[test]
    fn the_default_schema_is_recognised() {
        assert!(TenantId::parse("public").unwrap().is_default_schema());
        assert!(!TenantId::parse("tenant_1").unwrap().is_default_schema());
    }

    #[test]
    fn a_sharded_deployment_is_refused_rather_than_routed_to_the_default() {
        let config = DatabaseConfig {
            host: "localhost".into(),
            port: 5432,
            user: "postgres".into(),
            password: "password".into(),
            database: "postgres".into(),
            pool_size: 1,
            connect_timeout: Duration::from_secs(1),
            multi_tenant: true,
            sharded: true,
        };
        let error = Database::connect(&config).expect_err("sharding is refused");
        assert!(matches!(error, ConnectionError::Shard(_)), "{error}");
    }
}
