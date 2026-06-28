use anyhow::Result;
use hmm_core::Category;

pub trait CategoryRepository: Send + Sync {
    fn get(&self, category_id: &str) -> Result<Option<Category>>;
    fn save(&self, category: &Category) -> Result<()>;
    fn delete(&self, category_id: &str) -> Result<()>;
    fn list_all(&self) -> Result<Vec<Category>>;
    fn count_mods(&self, category_id: &str) -> Result<u32>;
    fn get_mod_categories(&self, mod_id: &str) -> Result<Vec<Category>>;
    fn set_mod_categories(&self, mod_id: &str, category_ids: &[String]) -> Result<()>;
    /// Returns all (mod_id, Category) pairs for batch merging in `get_mod_library`.
    fn list_mod_category_pairs(&self) -> Result<Vec<(String, Category)>>;
}
