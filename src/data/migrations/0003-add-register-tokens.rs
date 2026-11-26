use sea_orm::DbBackend;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "0003-add-register-tokens"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut id = ColumnDef::new(Alias::new("id"));
        let mut expires_at = ColumnDef::new(Alias::new("expires_at"));
        if manager.get_database_backend() == DbBackend::Postgres {
            id.uuid();
            expires_at.timestamp_with_time_zone();
        } else {
            id.text();
            expires_at.text();
        }
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("register_tokens"))
                    .if_not_exists()
                    .col(id.primary_key())
                    .col(expires_at.not_null())
                    .to_owned(),
            )
            .await
    }

    fn use_transaction(&self) -> Option<bool> {
        Some(false)
    }
}
