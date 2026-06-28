use anyhow::{bail, Result};
use hmm_core::Category;
use hmm_ports::{AppClock, CategoryRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct CategoryWithCount {
    pub category: Category,
    pub mod_count: u32,
}

pub struct CategoryService {
    category_repository: Arc<dyn CategoryRepository>,
    clock: Arc<dyn AppClock>,
}

impl CategoryService {
    pub fn new(
        category_repository: Arc<dyn CategoryRepository>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            category_repository,
            clock,
        }
    }

    pub fn create_category(
        &self,
        name: String,
        color: Option<String>,
        sort_order: Option<i32>,
    ) -> Result<String> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            bail!("category name must not be empty");
        }
        let color = color
            .map(|c| c.trim().to_owned())
            .filter(|c| !c.is_empty());

        let id = Uuid::new_v4().to_string();
        let now = self.clock.now_unix_millis()?;

        let category = Category {
            id: id.clone(),
            name,
            color,
            sort_order: sort_order.unwrap_or(0),
            created_at: now,
        };
        self.category_repository.save(&category)?;
        Ok(id)
    }

    pub fn update_category(
        &self,
        id: String,
        name: Option<String>,
        color: Option<Option<String>>,
        sort_order: Option<i32>,
    ) -> Result<()> {
        let existing = self
            .category_repository
            .get(&id)?
            .ok_or_else(|| anyhow::anyhow!("category not found: {id}"))?;

        let new_name = match name {
            Some(n) => {
                let trimmed = n.trim().to_owned();
                if trimmed.is_empty() {
                    bail!("category name must not be empty");
                }
                trimmed
            }
            None => existing.name,
        };

        let new_color = match color {
            Some(c) => c.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()),
            None => existing.color,
        };

        let updated = Category {
            id: existing.id,
            name: new_name,
            color: new_color,
            sort_order: sort_order.unwrap_or(existing.sort_order),
            created_at: existing.created_at,
        };
        self.category_repository.save(&updated)
    }

    pub fn delete_category(&self, id: &str) -> Result<()> {
        self.category_repository.delete(id)
    }

    pub fn list_categories(&self) -> Result<Vec<CategoryWithCount>> {
        let categories = self.category_repository.list_all()?;
        let mut result = Vec::with_capacity(categories.len());
        for cat in categories {
            let count = self.category_repository.count_mods(&cat.id)?;
            result.push(CategoryWithCount {
                category: cat,
                mod_count: count,
            });
        }
        Ok(result)
    }

    pub fn get_mod_categories(&self, mod_id: &str) -> Result<Vec<Category>> {
        self.category_repository.get_mod_categories(mod_id)
    }

    pub fn set_mod_categories(&self, mod_id: &str, category_ids: &[String]) -> Result<()> {
        self.category_repository
            .set_mod_categories(mod_id, category_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeCategoryRepository {
        categories: Mutex<Vec<Category>>,
        associations: Mutex<Vec<(String, String)>>, // (mod_id, category_id)
    }

    impl FakeCategoryRepository {
        fn new() -> Self {
            Self {
                categories: Mutex::new(Vec::new()),
                associations: Mutex::new(Vec::new()),
            }
        }
    }

    impl CategoryRepository for FakeCategoryRepository {
        fn get(&self, category_id: &str) -> Result<Option<Category>> {
            let cats = self.categories.lock().unwrap();
            Ok(cats.iter().find(|c| c.id == category_id).cloned())
        }

        fn save(&self, category: &Category) -> Result<()> {
            let mut cats = self.categories.lock().unwrap();
            cats.retain(|c| c.id != category.id);
            cats.push(category.clone());
            Ok(())
        }

        fn delete(&self, category_id: &str) -> Result<()> {
            let mut cats = self.categories.lock().unwrap();
            cats.retain(|c| c.id != category_id);
            let mut assocs = self.associations.lock().unwrap();
            assocs.retain(|(_, cid)| cid != category_id);
            Ok(())
        }

        fn list_all(&self) -> Result<Vec<Category>> {
            let cats = self.categories.lock().unwrap();
            let mut sorted = cats.clone();
            sorted.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then(a.name.cmp(&b.name)));
            Ok(sorted)
        }

        fn count_mods(&self, category_id: &str) -> Result<u32> {
            let assocs = self.associations.lock().unwrap();
            Ok(assocs.iter().filter(|(_, cid)| cid == category_id).count() as u32)
        }

        fn get_mod_categories(&self, mod_id: &str) -> Result<Vec<Category>> {
            let assocs = self.associations.lock().unwrap();
            let cats = self.categories.lock().unwrap();
            let ids: Vec<&str> = assocs
                .iter()
                .filter(|(mid, _)| mid == mod_id)
                .map(|(_, cid)| cid.as_str())
                .collect();
            Ok(cats.iter().filter(|c| ids.contains(&c.id.as_str())).cloned().collect())
        }

        fn set_mod_categories(&self, mod_id: &str, category_ids: &[String]) -> Result<()> {
            let mut assocs = self.associations.lock().unwrap();
            assocs.retain(|(mid, _)| mid != mod_id);
            for cid in category_ids {
                assocs.push((mod_id.to_owned(), cid.clone()));
            }
            Ok(())
        }

        fn list_mod_category_pairs(&self) -> Result<Vec<(String, Category)>> {
            let assocs = self.associations.lock().unwrap();
            let cats = self.categories.lock().unwrap();
            let mut pairs = Vec::new();
            for (mid, cid) in assocs.iter() {
                if let Some(cat) = cats.iter().find(|c| c.id == *cid) {
                    pairs.push((mid.clone(), cat.clone()));
                }
            }
            Ok(pairs)
        }
    }

    struct FixedClock(u128);

    impl AppClock for FixedClock {
        fn now_unix_millis(&self) -> Result<u128> {
            Ok(self.0)
        }
    }

    fn make_service() -> (CategoryService, Arc<FakeCategoryRepository>) {
        let repo = Arc::new(FakeCategoryRepository::new());
        let clock = Arc::new(FixedClock(5000));
        let service = CategoryService::new(Arc::clone(&repo) as _, clock);
        (service, repo)
    }

    #[test]
    fn create_category_returns_uuid_and_saves() {
        let (service, repo) = make_service();
        let id = service.create_category("Gameplay".to_owned(), None, None).unwrap();

        assert!(!id.is_empty());
        let saved = repo.get(&id).unwrap().expect("should exist");
        assert_eq!(saved.name, "Gameplay");
        assert!(saved.color.is_none());
        assert_eq!(saved.sort_order, 0);
        assert_eq!(saved.created_at, 5000);
    }

    #[test]
    fn create_category_trims_name() {
        let (service, repo) = make_service();
        let id = service.create_category("  Trimmed  ".to_owned(), None, None).unwrap();
        let saved = repo.get(&id).unwrap().unwrap();
        assert_eq!(saved.name, "Trimmed");
    }

    #[test]
    fn create_category_rejects_empty_name() {
        let (service, _) = make_service();
        assert!(service.create_category("   ".to_owned(), None, None).is_err());
    }

    #[test]
    fn create_category_with_color_and_sort_order() {
        let (service, repo) = make_service();
        let id = service
            .create_category("UI".to_owned(), Some("#FF0000".to_owned()), Some(5))
            .unwrap();
        let saved = repo.get(&id).unwrap().unwrap();
        assert_eq!(saved.color, Some("#FF0000".to_owned()));
        assert_eq!(saved.sort_order, 5);
    }

    #[test]
    fn update_category_merges_fields() {
        let (service, repo) = make_service();
        let id = service
            .create_category("Old".to_owned(), Some("#000".to_owned()), Some(1))
            .unwrap();

        service
            .update_category(id.clone(), Some("New".to_owned()), None, None)
            .unwrap();

        let saved = repo.get(&id).unwrap().unwrap();
        assert_eq!(saved.name, "New");
        assert_eq!(saved.color, Some("#000".to_owned())); // preserved
        assert_eq!(saved.sort_order, 1); // preserved
    }

    #[test]
    fn update_category_can_clear_color() {
        let (service, repo) = make_service();
        let id = service
            .create_category("Cat".to_owned(), Some("#FFF".to_owned()), None)
            .unwrap();

        service
            .update_category(id.clone(), None, Some(None), None)
            .unwrap();

        let saved = repo.get(&id).unwrap().unwrap();
        assert!(saved.color.is_none());
    }

    #[test]
    fn update_category_rejects_empty_name() {
        let (service, _) = make_service();
        let id = service.create_category("Cat".to_owned(), None, None).unwrap();
        assert!(service.update_category(id, Some("  ".to_owned()), None, None).is_err());
    }

    #[test]
    fn update_nonexistent_category_errors() {
        let (service, _) = make_service();
        assert!(service
            .update_category("bogus".to_owned(), Some("X".to_owned()), None, None)
            .is_err());
    }

    #[test]
    fn list_categories_includes_mod_count() {
        let (service, _) = make_service();
        let id = service.create_category("A".to_owned(), None, None).unwrap();
        service
            .set_mod_categories("mod-1", &[id.clone()])
            .unwrap();
        service
            .set_mod_categories("mod-2", &[id.clone()])
            .unwrap();

        let list = service.list_categories().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].mod_count, 2);
    }

    #[test]
    fn set_and_get_mod_categories() {
        let (service, _) = make_service();
        let id1 = service.create_category("A".to_owned(), None, None).unwrap();
        let id2 = service.create_category("B".to_owned(), None, None).unwrap();

        service
            .set_mod_categories("mod-1", &[id1.clone(), id2.clone()])
            .unwrap();

        let cats = service.get_mod_categories("mod-1").unwrap();
        assert_eq!(cats.len(), 2);
    }
}
