use sea_orm::DbBackend;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "0002-add-totp-fields"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let postgres = manager.get_database_backend() == DbBackend::Postgres;
        let table = if postgres {
            (Alias::new("public"), Alias::new("users")).into_table_ref()
        } else {
            Alias::new("users").into_table_ref()
        };
        manager
            .alter_table(
                Table::alter()
                    .table(table.clone())
                    .add_column(ColumnDef::new(Alias::new("totp_secret")).text())
                    .to_owned(),
            )
            .await?;
        let mut active = ColumnDef::new(Alias::new("totp_active"));
        if postgres {
            active.boolean().not_null().default(false);
        } else {
            active.integer().not_null().default(0);
        }
        manager
            .alter_table(
                Table::alter()
                    .table(table)
                    .add_column(&mut active)
                    .to_owned(),
            )
            .await
    }

    fn use_transaction(&self) -> Option<bool> {
        Some(false)
    }
}
