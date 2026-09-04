//! Compile-time migration script management

pub fn generate_migrations() -> anyhow::Result<()> {
    use std::path::PathBuf;
    let mut migrations = vec![];
    let turso_migrations = PathBuf::from("./migrations/turso");
    let postgres_migrations = PathBuf::from("./migrations/postgres");

    // Traverse all the migration entries
    for entry in std::fs::read_dir("./migrations/postgres")? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let filename = entry.file_name().into_string();
            if let Err(e) = filename {
                println!(
                    "cargo::error=migrations/postgres/{} contains invalid Unicode character!",
                    e.display()
                );
                panic!();
            }
            let filename = filename.unwrap();
            // check if Turso has it
            let turso_target = turso_migrations.join(&filename);
            if !turso_target.exists() {
                println!("cargo::error={} does not exist!", turso_target.display());
                panic!();
            }
            migrations.push(parse_filename(&filename));
        } else {
            println!(
                "cargo::warning=migrations/postgres/{} is not a file!",
                entry.file_name().display()
            );
        }
    }

    // Sort the migration entries
    migrations.sort_by(|this, that| Ord::cmp(&this.0, &that.0));

    // Generate final code
    let mut code = r#"mod migration_scripts {
    pub enum DatabaseType {
        Turso,
        Postgres,
    }

    #[repr(u16)]
    #[derive(Clone, Copy)]
    pub enum MigrationVersion {
    "#
    .to_string();

    for entry in migrations.iter() {
        code.push_str(&format!("{} = {},\n", entry.3, entry.0));
    }
    code.push_str(
        r#"
    }

    impl MigrationVersion {
    pub fn script(&self, db: DatabaseType) -> &'static str {
        match (self, db) {
    "#,
    );

    for entry in migrations.iter() {
        let postgres_file = std::fs::read_to_string(postgres_migrations.join(&entry.1))?;
        code.push_str(&format!(
            "(Self::{}, DatabaseType::Postgres) => r#\"{}\"#,\n",
            entry.3, postgres_file
        ));
        let turso_file = std::fs::read_to_string(turso_migrations.join(&entry.1))?;
        code.push_str(&format!(
            "(Self::{}, DatabaseType::Turso) => r#\"{}\"#,\n",
            entry.3, turso_file
        ));
    }

    code.push_str(
        r#"}
    }

    pub fn all() -> &'static [Self] {
    &[
    "#,
    );

    for entry in migrations.iter() {
        code.push_str(&format!("Self::{},\n", entry.3));
    }

    code.push_str(
        r#"]}

    pub fn id(&self) -> u16 {*self as u16}

    pub fn slug(&self) -> &'static str {
        match self {
"#,
    );
    for entry in migrations.iter() {
        code.push_str(&format!(
            "            Self::{} => \"{}\",\n",
            entry.3, entry.2
        ));
    }
    code.push_str(
        r#"        }
    }
}"#,
    );

    code.push_str("}\n");

    std::fs::write(
        PathBuf::from(std::env::var("OUT_DIR")?).join("migration_scripts.rs"),
        &code,
    )?;

    Ok(())
}
/// Parse a migration file, find its numeral ID, and return this ID and filename and slug and PascalCase name
fn parse_filename(filename: &str) -> (u16, String, String, String) {
    let input = filename.trim_end_matches(".sql");
    let splitted = input.split_once("-");
    if let Some((number, slug)) = splitted {
        let num_res = number.parse();
        if num_res.is_err() {
            println!(
                "cargo::error=Unable to parse migration file name: {} should be a number",
                number
            );
            panic!();
        }
        let number: u16 = num_res.unwrap();
        let pascal_case = slug
            .split('-')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        let mut result = first.to_uppercase().to_string();
                        result.extend(chars);
                        result
                    }
                }
            })
            .collect();
        (number, filename.to_string(), slug.to_string(), pascal_case)
    } else {
        println!(
            "cargo::error=Unable to parse migration file name: {}",
            input
        );
        panic!()
    }
}
