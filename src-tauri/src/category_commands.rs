use crate::dto::{CategoryDto, CategoryWithCountDto, CommandErrorDto};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn create_category(
    name: String,
    color: Option<String>,
    sort_order: Option<i32>,
    state: State<'_, AppState>,
) -> Result<String, CommandErrorDto> {
    state
        .categories
        .create_category(name, color, sort_order)
        .map_err(|e| CommandErrorDto {
            code: "category_error".to_owned(),
            message: e.to_string(),
        })
}

#[tauri::command]
pub fn update_category(
    category_id: String,
    name: Option<String>,
    color: Option<Option<String>>,
    sort_order: Option<i32>,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    state
        .categories
        .update_category(category_id, name, color, sort_order)
        .map_err(|e| CommandErrorDto {
            code: "category_error".to_owned(),
            message: e.to_string(),
        })
}

#[tauri::command]
pub fn delete_category(
    category_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    state
        .categories
        .delete_category(&category_id)
        .map_err(|e| CommandErrorDto {
            code: "category_error".to_owned(),
            message: e.to_string(),
        })
}

#[tauri::command]
pub fn list_categories(
    state: State<'_, AppState>,
) -> Result<Vec<CategoryWithCountDto>, CommandErrorDto> {
    state
        .categories
        .list_categories()
        .map(|cats| cats.into_iter().map(CategoryWithCountDto::from).collect())
        .map_err(|e| CommandErrorDto {
            code: "category_error".to_owned(),
            message: e.to_string(),
        })
}

#[tauri::command]
pub fn set_mod_categories(
    mod_id: String,
    category_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    state
        .categories
        .set_mod_categories(&mod_id, &category_ids)
        .map_err(|e| CommandErrorDto {
            code: "category_error".to_owned(),
            message: e.to_string(),
        })
}

#[tauri::command]
pub fn get_mod_categories(
    mod_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CategoryDto>, CommandErrorDto> {
    state
        .categories
        .get_mod_categories(&mod_id)
        .map(|cats| cats.into_iter().map(CategoryDto::from).collect())
        .map_err(|e| CommandErrorDto {
            code: "category_error".to_owned(),
            message: e.to_string(),
        })
}
