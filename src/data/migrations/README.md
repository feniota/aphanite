# Migrations

每次更改数据库结构时，都应该添加迁移脚本，否则程序无法运行。

下面详细介绍 Aphanite 的迁移系统。

## 数据模型的权威来源

Aphanite 使用 SeaORM 管理迁移，但是修改 entity 并不会自动同步数据库结构。

但是，我们约定，数据模型的真实类型以 Rust 代码中的类型为主；迁移脚本是为了让现存数据库中的数据模型“长得像” Rust 代码中的类型设计的。

## 迁移文件

Aphanite 的迁移是存放在 Rust 文件中的 SeaORM migration。由于我们支持 SQLite 和 Postgres 两种数据库后端，每次迁移都应该保证两种数据库后端均能正确执行。

> [!IMPORTANT]
>
> **永远**不要更改历史上的迁移文件。如有任何需要修改数据表结构的需求，请添加新迁移文件。

### 文件名

```
[数字 ID]-[迁移名称].rs
```

其中

- 数字 ID：虽然不做限制，但原则上应为四位数。沿用原来的约定，最大值是 65535。**迁移脚本的编写者应该严格保证该数值大于历史上所有迁移脚本的数值**，否则会导致逻辑错误。
- 迁移名称：应为纯 ASCII，以 kebab-case 命名的**唯一**迁移名称。

注意这两项都**不能重复**。目前由手动注册迁移保证唯一性，编译器不会检查这些约束。

### 存放路径

例如，我添加了名为 `0006-example.rs` 的迁移，那么目前的目录结构里，应该包含：

```
src/data/migrations/
└── 0006-example.rs
```

每次添加迁移后，都应该在 `src/data/migrations.rs` 中声明对应模块，并按数字 ID 顺序注册到 `Migrator::migrations()`，否则该迁移不会执行。

### 文件格式

每个迁移文件实现 `MigrationName` 和 `MigrationTrait`，使用 `SchemaManager` / SeaQuery 定义结构变更，必要的数据转换可以使用目标数据库对应的 SQL 方言。

同一个迁移文件中应处理 SQLite 和 Postgres 的差异。`MigrationName::name()` 必须与文件名去掉 `.rs` 后一致，例如 `0001-init`；已执行的迁移名称不能更改，否则会破坏已有版本记录。

但是注意：**不应该在迁移中写 `BEGIN TRANSACTION` 和 `COMMIT TRANSACTION` 语句，Aphanite 的迁移系统会自动加上**。`run_migrations` 统一管理事务，因此各迁移的 `use_transaction()` 应返回 `Some(false)`。
