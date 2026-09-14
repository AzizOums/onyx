//! Tenant scoping against a real PostgreSQL server.
//!
//! These need a live database. Set `LUMEN_DB_TEST=1` plus the usual
//! `POSTGRES_*` variables to run them; without it they report a skip rather
//! than failing, so `cargo test` still works on a machine with no server.
//!
//! The schemas they expect:
//!
//! ```sql
//! CREATE SCHEMA tenant_a;
//! CREATE SCHEMA "tenant-b";
//! CREATE TABLE tenant_a.widget   (id int primary key, owner text);
//! CREATE TABLE "tenant-b".widget (id int primary key, owner text);
//! CREATE TABLE public.widget     (id int primary key, owner text);
//! -- one row each, owner = 'belongs-to-a' / 'belongs-to-b' / 'belongs-to-public'
//! ```

use std::time::Duration;

use lumen_db::connection::{Database, DatabaseConfig, TenantId};

fn enabled() -> bool {
    std::env::var("LUMEN_DB_TEST").is_ok_and(|value| !value.is_empty())
}

fn config(multi_tenant: bool) -> DatabaseConfig {
    let mut config = DatabaseConfig::from_env();
    config.multi_tenant = multi_tenant;
    config.sharded = false;
    config.pool_size = 1; // One connection, so reuse is guaranteed and leaks show.
    config.connect_timeout = Duration::from_secs(5);
    config
}

async fn owner_of_widget(client: &deadpool_postgres::Object) -> String {
    let row = client
        .query_one("SELECT owner FROM widget WHERE id = 1", &[])
        .await
        .expect("the widget row is readable");
    row.get(0)
}

#[tokio::test]
async fn a_tenant_connection_reads_that_tenants_schema() {
    if !enabled() {
        eprintln!("skipped: set LUMEN_DB_TEST=1 and POSTGRES_* to run");
        return;
    }

    let database = Database::connect(&config(true)).expect("the pool builds");

    let tenant_a = TenantId::parse("tenant_a").unwrap();
    let client = database.tenant(&tenant_a).await.expect("a connection");
    assert_eq!(owner_of_widget(&client).await, "belongs-to-a");
    drop(client);

    // A dash is legal in a tenant id and illegal as a bare identifier, so this
    // case is what proves the quoting works against a real parser.
    let tenant_b = TenantId::parse("tenant-b").unwrap();
    let client = database.tenant(&tenant_b).await.expect("a connection");
    assert_eq!(owner_of_widget(&client).await, "belongs-to-b");
}

#[tokio::test]
async fn one_tenants_scope_does_not_survive_into_the_next_borrow() {
    if !enabled() {
        eprintln!("skipped: set LUMEN_DB_TEST=1 and POSTGRES_* to run");
        return;
    }

    // Pool size is 1, so the second borrow is certain to reuse the first
    // connection. `SET search_path` is session state: if it is not reset, the
    // second tenant reads the first tenant's rows.
    let database = Database::connect(&config(true)).expect("the pool builds");

    let tenant_a = TenantId::parse("tenant_a").unwrap();
    let client = database.tenant(&tenant_a).await.expect("a connection");
    assert_eq!(owner_of_widget(&client).await, "belongs-to-a");
    drop(client);

    let tenant_b = TenantId::parse("tenant-b").unwrap();
    let client = database.tenant(&tenant_b).await.expect("a connection");
    assert_eq!(
        owner_of_widget(&client).await,
        "belongs-to-b",
        "tenant-b read tenant_a's schema: search_path leaked across a pooled connection"
    );
}

#[tokio::test]
async fn an_unscoped_connection_never_inherits_a_tenants_scope() {
    if !enabled() {
        eprintln!("skipped: set LUMEN_DB_TEST=1 and POSTGRES_* to run");
        return;
    }

    // The dangerous ordering: a tenant borrow leaves search_path set, then
    // catalog work borrows the same connection and silently reads that tenant.
    let database = Database::connect(&config(true)).expect("the pool builds");

    let tenant_a = TenantId::parse("tenant_a").unwrap();
    let client = database.tenant(&tenant_a).await.expect("a connection");
    assert_eq!(owner_of_widget(&client).await, "belongs-to-a");
    drop(client);

    let client = database.unscoped().await.expect("a connection");
    assert_eq!(
        owner_of_widget(&client).await,
        "belongs-to-public",
        "an unscoped connection inherited tenant_a's search_path"
    );
}

#[tokio::test]
async fn the_self_hosted_shortcut_still_reads_the_default_schema() {
    if !enabled() {
        eprintln!("skipped: set LUMEN_DB_TEST=1 and POSTGRES_* to run");
        return;
    }

    // multi_tenant = false and the default schema: the Python code skips the
    // schema switch entirely, and the result must still be `public`.
    let database = Database::connect(&config(false)).expect("the pool builds");

    let public = TenantId::parse("public").unwrap();
    let client = database.tenant(&public).await.expect("a connection");
    assert_eq!(owner_of_widget(&client).await, "belongs-to-public");
}

#[tokio::test]
async fn a_missing_schema_fails_rather_than_falling_back() {
    if !enabled() {
        eprintln!("skipped: set LUMEN_DB_TEST=1 and POSTGRES_* to run");
        return;
    }

    // `SET search_path` accepts a schema that does not exist, so the failure
    // surfaces on the query. What matters is that it fails instead of quietly
    // resolving `widget` somewhere else.
    let database = Database::connect(&config(true)).expect("the pool builds");

    let ghost = TenantId::parse("tenant_missing").unwrap();
    let client = database.tenant(&ghost).await.expect("a connection");
    let result = client
        .query_one("SELECT owner FROM widget WHERE id = 1", &[])
        .await;
    assert!(
        result.is_err(),
        "a query against a missing schema resolved to some other schema"
    );
}
