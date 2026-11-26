use sea_orm::DbBackend;
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "0001-init"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let tables = [
            (
                "files",
                vec![
                    uuid_column("id", backend)
                        .not_null()
                        .primary_key()
                        .to_owned(),
                    timestamp_column("created_at", backend)
                        .not_null()
                        .to_owned(),
                    text_column("hash"),
                    text_column("data"),
                    ColumnDef::new(Alias::new("ref_count"))
                        .integer()
                        .not_null()
                        .to_owned(),
                ],
            ),
            (
                "game_profiles",
                vec![
                    uuid_column("id", backend)
                        .not_null()
                        .primary_key()
                        .to_owned(),
                    text_column("name"),
                    uuid_column("owner_id", backend).not_null().to_owned(),
                ],
            ),
            (
                "profile_textures",
                vec![
                    uuid_column("id", backend)
                        .not_null()
                        .primary_key()
                        .to_owned(),
                    timestamp_column("created_at", backend)
                        .not_null()
                        .to_owned(),
                    uuid_column("profile_id", backend).not_null().to_owned(),
                    ColumnDef::new(Alias::new("skin_model"))
                        .big_integer()
                        .not_null()
                        .to_owned(),
                    uuid_column("skin_file", backend),
                    uuid_column("cape_file", backend),
                ],
            ),
            (
                "users",
                vec![
                    uuid_column("id", backend)
                        .not_null()
                        .primary_key()
                        .to_owned(),
                    text_column("nickname"),
                    text_column("email"),
                    text_column("password"),
                    text_column("preferred_language"),
                    permission_column(backend),
                ],
            ),
            (
                "instances",
                vec![
                    uuid_column("id", backend)
                        .not_null()
                        .primary_key()
                        .to_owned(),
                    text_column("name"),
                    text_column("description"),
                    text_column("hash"),
                    uuid_column("file", backend).not_null().to_owned(),
                ],
            ),
            (
                "user_instances",
                vec![
                    uuid_column("id", backend)
                        .not_null()
                        .primary_key()
                        .to_owned(),
                    uuid_column("user_id", backend).not_null().to_owned(),
                    uuid_column("instance_id", backend).not_null().to_owned(),
                ],
            ),
            (
                "tokens",
                vec![
                    uuid_column("access_token", backend)
                        .not_null()
                        .primary_key()
                        .to_owned(),
                    uuid_column("user_id", backend).not_null().to_owned(),
                    text_column("client_token"),
                    timestamp_column("created_at", backend)
                        .not_null()
                        .to_owned(),
                    uuid_column("profile_id", backend),
                ],
            ),
        ];
        for (name, columns) in tables {
            let mut table = Table::create();
            table.table(table_ref(name, backend));
            if backend == DbBackend::Sqlite {
                table.if_not_exists();
            }
            for mut column in columns {
                table.col(&mut column);
            }
            manager.create_table(table).await?;
        }
        for (table, column, unique) in [
            ("files", "hash", true),
            ("game_profiles", "name", false),
            ("game_profiles", "owner_id", false),
            ("profile_textures", "profile_id", false),
            ("users", "email", true),
            ("user_instances", "user_id", false),
            ("user_instances", "instance_id", false),
            ("tokens", "user_id", false),
            ("tokens", "profile_id", false),
        ] {
            let mut index = Index::create();
            index
                .name(format!("index_{table}_by_{column}"))
                .table(table_ref(table, backend))
                .col(Alias::new(column));
            if unique {
                index.unique();
            }
            manager.create_index(index).await?;
        }
        Ok(())
    }

    fn use_transaction(&self) -> Option<bool> {
        Some(false) // run_migrations owns the transaction.
    }
}

fn table_ref(name: &str, backend: DbBackend) -> TableRef {
    if backend == DbBackend::Postgres {
        (Alias::new("public"), Alias::new(name)).into_table_ref()
    } else {
        Alias::new(name).into_table_ref()
    }
}

fn uuid_column(name: &str, backend: DbBackend) -> ColumnDef {
    let mut column = ColumnDef::new(Alias::new(name));
    if backend == DbBackend::Postgres {
        column.uuid();
    } else {
        column.blob();
    }
    column
}

fn timestamp_column(name: &str, backend: DbBackend) -> ColumnDef {
    let mut column = ColumnDef::new(Alias::new(name));
    if backend == DbBackend::Postgres {
        column.timestamp_with_time_zone();
    } else {
        column.text();
    }
    column
}

fn text_column(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name))
        .text()
        .not_null()
        .to_owned()
}

fn permission_column(backend: DbBackend) -> ColumnDef {
    let mut column = ColumnDef::new(Alias::new("permission"));
    if backend == DbBackend::Postgres {
        column.big_integer();
    } else {
        column.integer();
    }
    column.not_null().to_owned()
}
