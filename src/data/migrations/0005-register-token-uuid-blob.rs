use sea_orm::DbBackend;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "0005-register-token-uuid-blob"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // PostgreSQL already stores registration token IDs as native UUIDs.
        if manager.get_database_backend() == DbBackend::Postgres {
            return Ok(());
        }
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("new_register_tokens"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .blob()
                            .not_null()
                            .primary_key()
                            .check(Expr::cust("typeof(id) = 'blob' AND length(id) = 16")),
                    )
                    .col(ColumnDef::new(Alias::new("expires_at")).text().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO new_register_tokens (id, expires_at)
             SELECT CASE WHEN typeof(id) = 'blob' THEN id ELSE unhex(replace(id, '-', '')) END,
                    expires_at FROM register_tokens",
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("register_tokens"))
                    .to_owned(),
            )
            .await?;
        manager
            .rename_table(
                Table::rename()
                    .table(
                        Alias::new("new_register_tokens"),
                        Alias::new("register_tokens"),
                    )
                    .to_owned(),
            )
            .await
    }

    fn use_transaction(&self) -> Option<bool> {
        Some(false)
    }
}
