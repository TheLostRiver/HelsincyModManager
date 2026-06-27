use rusqlite_migration::{Migrations, M};

pub(crate) fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(include_str!(
        "migrations/001_metadata_categories.sql"
    ))])
}
