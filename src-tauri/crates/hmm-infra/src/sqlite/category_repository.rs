use anyhow::{anyhow, Context, Result};
use hmm_core::Category;
use hmm_ports::CategoryRepository;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct SqliteCategoryRepository {
    db: Arc<Mutex<Connection>>,
}

impl SqliteCategoryRepository {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self { db }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.db
            .lock()
            .map_err(|_| anyhow!("database lock poisoned"))
    }

    fn row_to_category(row: &rusqlite::Row<'_>) -> rusqlite::Result<Category> {
        Ok(Category {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            sort_order: row.get(3)?,
            created_at: row.get::<_, i64>(4)? as u128,
        })
    }
}

impl CategoryRepository for SqliteCategoryRepository {
    fn get(&self, category_id: &str) -> Result<Option<Category>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT category_id, name, color, sort_order, created_at
                 FROM categories WHERE category_id = ?1",
            )
            .context("failed to prepare get category query")?;

        let result = stmt.query_row(rusqlite::params![category_id], Self::row_to_category);

        match result {
            Ok(category) => Ok(Some(category)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error).context("failed to get category"),
        }
    }

    fn save(&self, category: &Category) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "INSERT OR REPLACE INTO categories
                (category_id, name, color, sort_order, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                category.id,
                category.name,
                category.color,
                category.sort_order,
                category.created_at as i64,
            ],
        )
        .context("failed to save category")?;
        Ok(())
    }

    fn delete(&self, category_id: &str) -> Result<()> {
        let conn = self.lock_db()?;
        conn.execute(
            "DELETE FROM categories WHERE category_id = ?1",
            rusqlite::params![category_id],
        )
        .context("failed to delete category")?;
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Category>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT category_id, name, color, sort_order, created_at
                 FROM categories ORDER BY sort_order ASC, name ASC",
            )
            .context("failed to prepare list categories query")?;

        let rows = stmt
            .query_map([], Self::row_to_category)
            .context("failed to list categories")?;

        let mut categories = Vec::new();
        for row in rows {
            categories.push(row.context("failed to read category row")?);
        }
        Ok(categories)
    }

    fn count_mods(&self, category_id: &str) -> Result<u32> {
        let conn = self.lock_db()?;
        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM mod_categories WHERE category_id = ?1",
                rusqlite::params![category_id],
                |row| row.get(0),
            )
            .context("failed to count mods for category")?;
        Ok(count)
    }

    fn get_mod_categories(&self, mod_id: &str) -> Result<Vec<Category>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT c.category_id, c.name, c.color, c.sort_order, c.created_at
                 FROM categories c
                 INNER JOIN mod_categories mc ON mc.category_id = c.category_id
                 WHERE mc.mod_id = ?1
                 ORDER BY c.sort_order ASC, c.name ASC",
            )
            .context("failed to prepare get mod categories query")?;

        let rows = stmt
            .query_map(rusqlite::params![mod_id], Self::row_to_category)
            .context("failed to get mod categories")?;

        let mut categories = Vec::new();
        for row in rows {
            categories.push(row.context("failed to read mod category row")?);
        }
        Ok(categories)
    }

    fn set_mod_categories(&self, mod_id: &str, category_ids: &[String]) -> Result<()> {
        let conn = self.lock_db()?;
        let tx = conn
            .unchecked_transaction()
            .context("failed to begin transaction")?;

        tx.execute(
            "DELETE FROM mod_categories WHERE mod_id = ?1",
            rusqlite::params![mod_id],
        )
        .context("failed to clear old mod categories")?;

        for category_id in category_ids {
            tx.execute(
                "INSERT INTO mod_categories (mod_id, category_id) VALUES (?1, ?2)",
                rusqlite::params![mod_id, category_id],
            )
            .context("failed to insert mod category association")?;
        }

        tx.commit().context("failed to commit mod categories")?;
        Ok(())
    }

    fn list_mod_category_pairs(&self) -> Result<Vec<(String, Category)>> {
        let conn = self.lock_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT mc.mod_id, c.category_id, c.name, c.color, c.sort_order, c.created_at
                 FROM mod_categories mc
                 INNER JOIN categories c ON c.category_id = mc.category_id
                 ORDER BY c.sort_order ASC, c.name ASC",
            )
            .context("failed to prepare list mod category pairs query")?;

        let rows = stmt
            .query_map([], |row| {
                let mod_id: String = row.get(0)?;
                let category = Category {
                    id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    sort_order: row.get(4)?,
                    created_at: row.get::<_, i64>(5)? as u128,
                };
                Ok((mod_id, category))
            })
            .context("failed to list mod category pairs")?;

        let mut pairs = Vec::new();
        for row in rows {
            pairs.push(row.context("failed to read mod category pair row")?);
        }
        Ok(pairs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite::open_database;

    fn test_repo() -> SqliteCategoryRepository {
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");
        let conn = open_database(&db_path).unwrap();
        std::mem::forget(temp);
        SqliteCategoryRepository::new(Arc::new(Mutex::new(conn)))
    }

    fn sample_category(id: &str, name: &str) -> Category {
        Category {
            id: id.to_owned(),
            name: name.to_owned(),
            color: None,
            sort_order: 0,
            created_at: 1000,
        }
    }

    #[test]
    fn get_returns_none_for_missing() {
        let repo = test_repo();
        assert!(repo.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn save_and_get_round_trips() {
        let repo = test_repo();
        let cat = Category {
            id: "cat-1".to_owned(),
            name: "Gameplay".to_owned(),
            color: Some("#FF0000".to_owned()),
            sort_order: 5,
            created_at: 2000,
        };

        repo.save(&cat).unwrap();
        let loaded = repo.get("cat-1").unwrap().expect("should exist");

        assert_eq!(loaded.id, "cat-1");
        assert_eq!(loaded.name, "Gameplay");
        assert_eq!(loaded.color, Some("#FF0000".to_owned()));
        assert_eq!(loaded.sort_order, 5);
        assert_eq!(loaded.created_at, 2000);
    }

    #[test]
    fn save_replaces_existing() {
        let repo = test_repo();
        let mut cat = sample_category("cat-1", "Old Name");
        repo.save(&cat).unwrap();

        cat.name = "New Name".to_owned();
        cat.color = Some("#00FF00".to_owned());
        repo.save(&cat).unwrap();

        let loaded = repo.get("cat-1").unwrap().expect("should exist");
        assert_eq!(loaded.name, "New Name");
        assert_eq!(loaded.color, Some("#00FF00".to_owned()));
    }

    #[test]
    fn delete_removes_category() {
        let repo = test_repo();
        repo.save(&sample_category("cat-1", "Test")).unwrap();
        repo.delete("cat-1").unwrap();
        assert!(repo.get("cat-1").unwrap().is_none());
    }

    #[test]
    fn delete_cascades_to_mod_categories() {
        let repo = test_repo();
        repo.save(&sample_category("cat-1", "Test")).unwrap();
        repo.set_mod_categories("mod-1", &["cat-1".to_owned()])
            .unwrap();

        repo.delete("cat-1").unwrap();

        let cats = repo.get_mod_categories("mod-1").unwrap();
        assert!(cats.is_empty());
    }

    #[test]
    fn list_all_orders_by_sort_order_then_name() {
        let repo = test_repo();
        let mut c1 = sample_category("c1", "Zebra");
        c1.sort_order = 1;
        let mut c2 = sample_category("c2", "Alpha");
        c2.sort_order = 1;
        let mut c3 = sample_category("c3", "Beta");
        c3.sort_order = 0;

        repo.save(&c1).unwrap();
        repo.save(&c2).unwrap();
        repo.save(&c3).unwrap();

        let all = repo.list_all().unwrap();
        let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Beta", "Alpha", "Zebra"]);
    }

    #[test]
    fn count_mods_returns_correct_count() {
        let repo = test_repo();
        repo.save(&sample_category("cat-1", "Test")).unwrap();
        repo.set_mod_categories("mod-1", &["cat-1".to_owned()])
            .unwrap();
        repo.set_mod_categories("mod-2", &["cat-1".to_owned()])
            .unwrap();

        assert_eq!(repo.count_mods("cat-1").unwrap(), 2);
        assert_eq!(repo.count_mods("nonexistent").unwrap(), 0);
    }

    #[test]
    fn set_mod_categories_replaces_all() {
        let repo = test_repo();
        repo.save(&sample_category("cat-1", "A")).unwrap();
        repo.save(&sample_category("cat-2", "B")).unwrap();
        repo.save(&sample_category("cat-3", "C")).unwrap();

        repo.set_mod_categories("mod-1", &["cat-1".to_owned(), "cat-2".to_owned()])
            .unwrap();
        let cats = repo.get_mod_categories("mod-1").unwrap();
        assert_eq!(cats.len(), 2);

        // Replace with different set
        repo.set_mod_categories("mod-1", &["cat-3".to_owned()])
            .unwrap();
        let cats = repo.get_mod_categories("mod-1").unwrap();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].id, "cat-3");
    }

    #[test]
    fn set_mod_categories_empty_clears_all() {
        let repo = test_repo();
        repo.save(&sample_category("cat-1", "A")).unwrap();
        repo.set_mod_categories("mod-1", &["cat-1".to_owned()])
            .unwrap();

        repo.set_mod_categories("mod-1", &[]).unwrap();
        let cats = repo.get_mod_categories("mod-1").unwrap();
        assert!(cats.is_empty());
    }

    #[test]
    fn list_mod_category_pairs_returns_all_associations() {
        let repo = test_repo();
        repo.save(&sample_category("cat-1", "A")).unwrap();
        repo.save(&sample_category("cat-2", "B")).unwrap();
        repo.set_mod_categories("mod-1", &["cat-1".to_owned(), "cat-2".to_owned()])
            .unwrap();
        repo.set_mod_categories("mod-2", &["cat-1".to_owned()])
            .unwrap();

        let pairs = repo.list_mod_category_pairs().unwrap();
        assert_eq!(pairs.len(), 3);

        let mod1_cats: Vec<&str> = pairs
            .iter()
            .filter(|(m, _)| m == "mod-1")
            .map(|(_, c)| c.name.as_str())
            .collect();
        assert_eq!(mod1_cats.len(), 2);

        let mod2_cats: Vec<&str> = pairs
            .iter()
            .filter(|(m, _)| m == "mod-2")
            .map(|(_, c)| c.name.as_str())
            .collect();
        assert_eq!(mod2_cats, vec!["A"]);
    }
}
