use sea_orm::DbBackend;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "0004-remove-otp-active"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DbBackend::Postgres {
            return manager
                .alter_table(
                    Table::alter()
                        .table((Alias::new("public"), Alias::new("users")))
                        .drop_column(Alias::new("totp_active"))
                        .to_owned(),
                )
                .await;
        }
        // Preserve the historical SQLite rebuild, including its index behavior.
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("new_users"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .blob()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("nickname")).text().not_null())
                    .col(ColumnDef::new(Alias::new("email")).text().not_null())
                    .col(ColumnDef::new(Alias::new("password")).text().not_null())
                    .col(
                        ColumnDef::new(Alias::new("preferred_language"))
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("permission"))
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("totp_secret")).text())
                    .to_owned(),
            )
            .await?;
        manager.get_connection().execute_unprepared(
            "INSERT INTO new_users (id, nickname, email, password, preferred_language, permission, totp_secret)
             SELECT id, nickname, email, password, preferred_language, permission, totp_secret FROM users"
        ).await?;
        manager
            .drop_table(Table::drop().table(Alias::new("users")).to_owned())
            .await?;
        manager
            .rename_table(
                Table::rename()
                    .table(Alias::new("new_users"), Alias::new("users"))
                    .to_owned(),
            )
            .await
    }

    fn use_transaction(&self) -> Option<bool> {
        Some(false)
    }
}
