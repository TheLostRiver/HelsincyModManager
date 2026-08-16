use std::collections::BTreeSet;

pub(crate) const MAX_SAVE_DIRECTORY_COUNT: usize = 512;
pub(crate) const MAX_SAVE_PATH_COMPONENTS: usize = 32;
pub(crate) const MAX_SAVE_PATH_COMPONENT_BYTES: usize = 255;
pub(crate) const MAX_SAVE_RELATIVE_PATH_BYTES: usize = 1024;

pub(crate) fn normalize_save_relative_path(raw: &str) -> Option<String> {
    if raw.is_empty()
        || raw.len() > MAX_SAVE_RELATIVE_PATH_BYTES
        || raw.starts_with('/')
        || raw.starts_with('\\')
        || raw.contains('\0')
    {
        return None;
    }

    let normalized = raw.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        if parts.len() >= MAX_SAVE_PATH_COMPONENTS
            || part.is_empty()
            || part.len() > MAX_SAVE_PATH_COMPONENT_BYTES
            || part == "."
            || part == ".."
            || part.contains(':')
            || part.ends_with('.')
            || part.ends_with(' ')
            || part.chars().any(char::is_control)
            || is_windows_device_name(part)
        {
            return None;
        }
        parts.push(part);
    }

    Some(parts.join("/"))
}

pub(crate) fn record_parent_directories(
    relative_path: &str,
    directories: &mut BTreeSet<String>,
) -> bool {
    let mut prefix = String::new();
    let mut components = relative_path.split('/').peekable();
    while let Some(component) = components.next() {
        if components.peek().is_none() {
            break;
        }
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        directories.insert(prefix.clone());
        if directories.len() > MAX_SAVE_DIRECTORY_COUNT {
            return false;
        }
    }
    true
}

fn is_windows_device_name(part: &str) -> bool {
    let stem = part.split('.').next().unwrap_or(part).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_relative_path_budget_accepts_boundary_and_rejects_excess() {
        let boundary = (0..MAX_SAVE_PATH_COMPONENTS)
            .map(|index| format!("p{index}"))
            .collect::<Vec<_>>()
            .join("/");
        assert_eq!(
            normalize_save_relative_path(&boundary).as_deref(),
            Some(boundary.as_str())
        );

        let too_deep = format!("{boundary}/extra");
        assert!(normalize_save_relative_path(&too_deep).is_none());
        assert!(
            normalize_save_relative_path(&"x".repeat(MAX_SAVE_PATH_COMPONENT_BYTES + 1)).is_none()
        );
        assert!(
            normalize_save_relative_path(&"x".repeat(MAX_SAVE_RELATIVE_PATH_BYTES + 1)).is_none()
        );
    }

    #[test]
    fn parent_directory_budget_counts_unique_prefixes() {
        let mut directories = BTreeSet::new();
        for index in 0..MAX_SAVE_DIRECTORY_COUNT {
            assert!(record_parent_directories(
                &format!("directory-{index}/save.bin"),
                &mut directories,
            ));
        }
        assert!(!record_parent_directories(
            "directory-overflow/save.bin",
            &mut directories,
        ));
    }
}
