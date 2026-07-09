//! Database schema management and migrations
//!
//! Toasty's migration is complex and unstable, and requires a standalone binary,
//! which is not that suitable for us. Also, when it comes to backfilling, especially
//! for those data which cannot be calculated in pure SQL, we will need to make our own
//! "updates" system eventually. So given these circumstances, here we implement
//! a simple migration system.
//!
//! Migrations use raw connections (rusqlite / tokio_postgres) to run **before** toasty
//! ORM starts, so that the schema is in the correct state.
//!
//! Each migration is wrapped in a database transaction automatically — migration SQL
//! files should NOT contain BEGIN/COMMIT statements.

use crate::config::DatabaseBackend;
use anyhow::Context;

/// Run all pending migrations.
///
/// This must be called **before** toasty ORM connects to the database,
/// so that the schema is in the correct state before the ORM starts.
pub async fn init(config: &crate::config::AppConfig) -> anyhow::Result<()> {
    match config.database.backend {
        DatabaseBackend::Sqlite => run_sqlite(config).await,
        DatabaseBackend::Postgres => run_postgres(config).await,
    }
}

async fn run_sqlite(config: &crate::config::AppConfig) -> anyhow::Result<()> {
    let db_path = config.service.data_path.join("db.sqlite");
    let mut conn = rusqlite::Connection::open(&db_path)
        .with_context(|| format!("Failed to open SQLite database at {}", db_path.display()))?;

    // Set busy timeout to avoid contention in edge cases
    conn.execute_batch("PRAGMA busy_timeout = 5000;")
        .context("Failed to set busy_timeout")?;

    // Create the internal meta table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS __aphanite_migrations (
            id INTEGER PRIMARY KEY,
            slug TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
    .context("Failed to create __aphanite_migrations table")?;

    // Collect already-applied migration IDs
    let applied: Vec<u16> = {
        let mut stmt = conn
            .prepare("SELECT id FROM __aphanite_migrations ORDER BY id")
            .context("Failed to prepare query for applied migrations")?;
        stmt.query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to read applied migrations")?
    };
    // stmt is dropped here, releasing the immutable borrow on conn

    // Apply pending migrations in order
    for m in migration_scripts::MigrationVersion::all() {
        if applied.contains(&m.id()) {
            continue;
        }

        let slug = m.slug();
        let sql = m.script(migration_scripts::DatabaseType::Sqlite);

        let tx = conn.transaction().with_context(|| {
            format!(
                "Failed to begin transaction for migration {} ({})",
                m.id(),
                slug
            )
        })?;

        tx.execute_batch(sql)
            .with_context(|| format!("Migration {} ({}) failed", m.id(), slug))?;

        tx.execute(
            "INSERT INTO __aphanite_migrations (id, slug) VALUES (?1, ?2)",
            rusqlite::params![m.id(), slug],
        )
        .with_context(|| format!("Failed to record migration {} ({})", m.id(), slug))?;

        tx.commit()
            .with_context(|| format!("Failed to commit migration {} ({})", m.id(), slug))?;

        tracing::info!("Applied migration {} ({})", m.id(), slug);
    }

    Ok(())
}

async fn run_postgres(config: &crate::config::AppConfig) -> anyhow::Result<()> {
    let (mut client, connection) =
        tokio_postgres::connect(&config.database.postgres_url, tokio_postgres::NoTls)
            .await
            .context("Failed to connect to PostgreSQL")?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!("PostgreSQL connection error: {e}");
        }
    });

    // Acquire a session-level advisory lock so that concurrent instances
    // do not run migrations simultaneously.
    let locked = client
        .query("SELECT pg_try_advisory_lock(0, 0)", &[])
        .await
        .context("Failed to acquire migration advisory lock")?;
    let acquired: bool = locked[0].get(0);
    if !acquired {
        anyhow::bail!(
            "Another Aphanite instance is currently running migrations. \
             If this is a mistake, run: SELECT pg_advisory_unlock(0, 0)"
        );
    }

    // Create the internal meta table
    client
        .batch_execute(
            "CREATE TABLE IF NOT EXISTS __aphanite_migrations (
                id INTEGER PRIMARY KEY,
                slug TEXT NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );",
        )
        .await
        .context("Failed to create __aphanite_migrations table")?;

    // Collect already-applied migration IDs
    let rows = client
        .query("SELECT id FROM __aphanite_migrations ORDER BY id", &[])
        .await
        .context("Failed to query applied migrations")?;
    let applied: Vec<u16> = rows
        .iter()
        .map(|row| {
            let id: i32 = row.get(0);
            id as u16
        })
        .collect();

    // Apply pending migrations in order
    for m in migration_scripts::MigrationVersion::all() {
        if applied.contains(&m.id()) {
            continue;
        }

        let slug = m.slug();
        let sql = m.script(migration_scripts::DatabaseType::Postgres);

        let tx = client.transaction().await.with_context(|| {
            format!(
                "Failed to begin transaction for migration {} ({})",
                m.id(),
                slug
            )
        })?;

        // Use simple_query which handles multi-statement SQL
        tx.batch_execute(sql)
            .await
            .with_context(|| format!("Migration {} ({}) failed", m.id(), slug))?;

        tx.execute(
            "INSERT INTO __aphanite_migrations (id, slug) VALUES ($1, $2)",
            &[&(m.id() as i32), &slug],
        )
        .await
        .with_context(|| format!("Failed to record migration {} ({})", m.id(), slug))?;

        tx.commit()
            .await
            .with_context(|| format!("Failed to commit migration {} ({})", m.id(), slug))?;

        tracing::info!("Applied migration {} ({})", m.id(), slug);
    }

    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/", "migration_scripts.rs"));
