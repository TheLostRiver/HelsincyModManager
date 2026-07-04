use rusqlite_migration::{Migrations, M};

pub(crate) fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("migrations/001_metadata_categories.sql")),
        M::up(include_str!("migrations/002_profiles.sql")),
        M::up(include_str!("migrations/003_profile_save_settings.sql")),
        M::up(include_str!("migrations/004_save_backups.sql")),
        M::up(include_str!(
            "migrations/005_save_backup_directory_snapshot.sql"
        )),
    ])
}
