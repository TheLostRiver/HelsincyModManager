use super::key_values::{parse_key_values, KeyValuesError, KeyValueNode};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamLibraryFolder {
    pub path: PathBuf,
    pub app_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamAppManifest {
    pub app_id: u32,
    pub install_dir: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SteamManifestError {
    #[error("key values parse failed: {0}")]
    Parse(#[from] KeyValuesError),
    #[error("expected object: {0}")]
    ExpectedObject(&'static str),
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("invalid app id: {0}")]
    InvalidAppId(String),
}

pub fn parse_library_folders(input: &str) -> Result<Vec<SteamLibraryFolder>, SteamManifestError> {
    let root = parse_key_values(input)?;
    let libraryfolders = object_field(&root, "libraryfolders")?;
    let mut folders = Vec::new();

    for (key, folder_node) in libraryfolders {
        if key.parse::<u32>().is_err() {
            continue;
        }

        let folder = object_from_node(folder_node, "library folder")?;
        let path = text_field(folder, "path")?;
        let app_ids = match folder.get("apps") {
            Some(apps) => object_from_node(apps, "apps")?
                .keys()
                .filter_map(|app_id| app_id.parse::<u32>().ok())
                .collect(),
            None => Vec::new(),
        };

        folders.push(SteamLibraryFolder {
            path: PathBuf::from(path),
            app_ids,
        });
    }

    Ok(folders)
}

pub fn parse_app_manifest(input: &str) -> Result<SteamAppManifest, SteamManifestError> {
    let root = parse_key_values(input)?;
    let app_state = object_field(&root, "AppState")?;
    let app_id_text = text_field(app_state, "appid")?;
    let app_id = app_id_text
        .parse::<u32>()
        .map_err(|_| SteamManifestError::InvalidAppId(app_id_text.to_owned()))?;
    let install_dir = text_field(app_state, "installdir")?.to_owned();

    Ok(SteamAppManifest {
        app_id,
        install_dir,
    })
}

fn object_field<'a>(
    node: &'a KeyValueNode,
    field: &'static str,
) -> Result<&'a std::collections::BTreeMap<String, KeyValueNode>, SteamManifestError> {
    let object = object_from_node(node, "root")?;
    let value = object
        .get(field)
        .ok_or(SteamManifestError::MissingField(field))?;

    object_from_node(value, field)
}

fn object_from_node<'a>(
    node: &'a KeyValueNode,
    label: &'static str,
) -> Result<&'a std::collections::BTreeMap<String, KeyValueNode>, SteamManifestError> {
    match node {
        KeyValueNode::Object(object) => Ok(object),
        KeyValueNode::Text(_) => Err(SteamManifestError::ExpectedObject(label)),
    }
}

fn text_field<'a>(
    object: &'a std::collections::BTreeMap<String, KeyValueNode>,
    field: &'static str,
) -> Result<&'a str, SteamManifestError> {
    match object.get(field) {
        Some(KeyValueNode::Text(value)) => Ok(value),
        Some(KeyValueNode::Object(_)) => Err(SteamManifestError::ExpectedObject(field)),
        None => Err(SteamManifestError::MissingField(field)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_manifest_parses_library_folders_with_target_app() {
        let folders = parse_library_folders(
            r#"
            "libraryfolders"
            {
                "0"
                {
                    "path" "D:\\SteamLibrary"
                    "apps"
                    {
                        "582010" "123456"
                    }
                }
            }
            "#,
        )
        .expect("library folders");

        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].app_ids, vec![582010]);
    }

    #[test]
    fn steam_manifest_parses_app_manifest_install_dir() {
        let manifest = parse_app_manifest(
            r#"
            "AppState"
            {
                "appid" "582010"
                "installdir" "Monster Hunter World"
            }
            "#,
        )
        .expect("app manifest");

        assert_eq!(manifest.app_id, 582010);
        assert_eq!(manifest.install_dir, "Monster Hunter World");
    }
}
