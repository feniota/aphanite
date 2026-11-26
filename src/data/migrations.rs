//! SeaORM migrations with compatibility for historical migration records.
//!
//! Existing `__aphanite_migrations` records are imported into `seaql_migrations`.
//! The legacy table is retained unchanged; new migrations are tracked by SeaORM.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, Set, TransactionTrait,
    sea_query::{Alias, Expr, OnConflict, Query},
};
use sea_orm_migration::{
    MigrationTrait, MigratorTrait, SchemaManager, async_trait::async_trait, seaql_migrations,
};

#[path = "migrations/0001-init.rs"]
mod m0001_init;
#[path = "migrations/0002-add-totp-fields.rs"]
mod m0002_add_totp_fields;
#[path = "migrations/0003-add-register-tokens.rs"]
mod m0003_add_register_tokens;
#[path = "migrations/0004-remove-otp-active.rs"]
mod m0004_remove_otp_active;
#[path = "migrations/0005-register-token-uuid-blob.rs"]
mod m0005_register_token_uuid_blob;

/// Apply pending migrations before making any application queries.
///
/// Importing legacy records, changing the schema, and recording versions are atomic.
/// PostgreSQL uses a transaction-scoped advisory lock; SQLite takes its write lock
/// before reading migration records.
pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    let tx = db.begin().await?;
    match tx.get_database_backend() {
        DbBackend::Postgres => {
            let row = tx
                .query_one(
                    &Query::select()
                        .expr(Expr::cust("pg_try_advisory_xact_lock(0, 0)"))
                        .to_owned(),
                )
                .await?
                .ok_or_else(|| DbErr::Migration("Missing migration lock result".into()))?;
            if !row.try_get_by_index::<bool>(0)? {
                return Err(DbErr::Migration(
                    "Another Aphanite instance is currently running migrations".into(),
                ));
            }
        }
        DbBackend::Sqlite => {
            Migrator::install(&tx).await?;
            // Acquire a write lock even when the tracking table already exists.
            tx.execute_unprepared(
                "UPDATE seaql_migrations SET applied_at = applied_at WHERE 1 = 0",
            )
            .await?;
        }
        _ => return Err(DbErr::Migration("Unsupported database backend".into())),
    }

    Migrator::install(&tx).await?;
    let manager = SchemaManager::new(&tx);
    if manager.has_table("__aphanite_migrations").await? {
        let records = tx
            .query_all(
                &Query::select()
                    .columns([Alias::new("id"), Alias::new("slug")])
                    .from(Alias::new("__aphanite_migrations"))
                    .to_owned(),
            )
            .await?;
        for record in records {
            let id: i32 = record.try_get("", "id")?;
            let slug: String = record.try_get("", "slug")?;
            let version = format!("{id:04}-{slug}");
            if !Migrator::migrations()
                .iter()
                .any(|migration| migration.name() == version)
            {
                return Err(DbErr::Migration(format!(
                    "Unknown legacy migration {id} ({slug})"
                )));
            }
            seaql_migrations::Entity::insert(seaql_migrations::ActiveModel {
                version: Set(version),
                applied_at: Set(sea_orm::prelude::ChronoUtc::now().timestamp()),
            })
            .on_conflict(
                OnConflict::column(seaql_migrations::Column::Version)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(&tx)
            .await?;
        }
    }
    Migrator::up(&tx, None).await?;
    tx.commit().await
}

struct Migrator;

#[async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m0001_init::Migration),
            Box::new(m0002_add_totp_fields::Migration),
            Box::new(m0003_add_register_tokens::Migration),
            Box::new(m0004_remove_otp_active::Migration),
            Box::new(m0005_register_token_uuid_blob::Migration),
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, PaginatorTrait};

    #[ntex::test]
    async fn fresh_database_and_repeated_runs() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        run_migrations(&db).await.unwrap();
        run_migrations(&db).await.unwrap();
        assert_eq!(
            seaql_migrations::Entity::find().count(&db).await.unwrap(),
            Migrator::migrations().len() as u64,
        );
        let manager = SchemaManager::new(&db);
        assert!(manager.has_table("users").await.unwrap());
        assert!(manager.has_column("users", "totp_secret").await.unwrap());
        assert!(!manager.has_column("users", "totp_active").await.unwrap());
    }

    #[ntex::test]
    async fn legacy_records_and_token_data_are_preserved() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE __aphanite_migrations (
                id INTEGER PRIMARY KEY, slug TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .await
        .unwrap();
        for migration in Migrator::migrations().into_iter().take(4) {
            migration.up(&SchemaManager::new(&db)).await.unwrap();
            let (id, slug) = migration.name().split_once('-').unwrap();
            let id: i32 = id.parse().unwrap();
            db.execute(
                &Query::insert()
                    .into_table(Alias::new("__aphanite_migrations"))
                    .columns([Alias::new("id"), Alias::new("slug")])
                    .values_panic([id.into(), slug.into()])
                    .to_owned(),
            )
            .await
            .unwrap();
        }
        db.execute_unprepared(
            "INSERT INTO register_tokens VALUES (
                '018f0000-0000-7000-8000-000000000001', '2099-01-01T00:00:00Z'
            )",
        )
        .await
        .unwrap();
        let before = db
            .query_all(
                &Query::select()
                    .expr(Expr::cust("*"))
                    .from(Alias::new("__aphanite_migrations"))
                    .order_by(Alias::new("id"), sea_orm::Order::Asc)
                    .to_owned(),
            )
            .await
            .unwrap();

        run_migrations(&db).await.unwrap();
        run_migrations(&db).await.unwrap();

        let after = db
            .query_all(
                &Query::select()
                    .expr(Expr::cust("*"))
                    .from(Alias::new("__aphanite_migrations"))
                    .order_by(Alias::new("id"), sea_orm::Order::Asc)
                    .to_owned(),
            )
            .await
            .unwrap();
        assert_eq!(before.len(), after.len());
        for (before, after) in before.iter().zip(&after) {
            assert_eq!(
                before.try_get::<i32>("", "id").unwrap(),
                after.try_get::<i32>("", "id").unwrap()
            );
            for column in ["slug", "applied_at"] {
                assert_eq!(
                    before.try_get::<String>("", column).unwrap(),
                    after.try_get::<String>("", column).unwrap()
                );
            }
        }
        let token = crate::data::entity::register_tokens::Entity::find()
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(token.id.to_string(), "018f0000-0000-7000-8000-000000000001");
        assert_eq!(token.expires_at.to_rfc3339(), "2099-01-01T00:00:00+00:00");
        assert_eq!(
            seaql_migrations::Entity::find().count(&db).await.unwrap(),
            Migrator::migrations().len() as u64
        );
    }

    #[ntex::test]
    async fn failed_migration_rolls_back_schema_and_records() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE seaql_migrations (
                version TEXT PRIMARY KEY CHECK (version != '0002-add-totp-fields'),
                applied_at BIGINT NOT NULL
            )",
        )
        .await
        .unwrap();
        assert!(run_migrations(&db).await.is_err());
        assert!(!SchemaManager::new(&db).has_table("users").await.unwrap());
        assert_eq!(
            seaql_migrations::Entity::find().count(&db).await.unwrap(),
            0
        );
    }
}
