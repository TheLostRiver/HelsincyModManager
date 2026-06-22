use anyhow::Result;
use hmm_ports::{ThumbnailRef, ThumbnailStore};
use std::io::Write;
use std::path::PathBuf;

pub struct FileSystemThumbnailStore {
    root_dir: PathBuf,
}

impl FileSystemThumbnailStore {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }
}

impl ThumbnailStore for FileSystemThumbnailStore {
    fn put_thumbnail(
        &self,
        package_id: &str,
        content_hash: &str,
        extension: &str,
        bytes: &[u8],
    ) -> Result<ThumbnailRef> {
        let safe_package_id = sanitize_path_segment(package_id);
        let safe_hash = sanitize_path_segment(content_hash);
        let safe_extension = sanitize_path_segment(extension);
        let variant = "preview-768".to_owned();
        let package_dir = self.root_dir.join("thumbnails").join(&safe_package_id);
        std::fs::create_dir_all(&package_dir)?;

        let final_path = package_dir.join(format!("{variant}-{safe_hash}.{safe_extension}"));
        if !final_path.exists() {
            let mut temp_file = tempfile::NamedTempFile::new_in(&package_dir)?;
            temp_file.write_all(bytes)?;
            temp_file.flush()?;
            match temp_file.persist_noclobber(&final_path) {
                Ok(_) => {}
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.error.into()),
            }
        }

        Ok(ThumbnailRef {
            package_id: safe_package_id,
            content_hash: safe_hash,
            variant,
        })
    }

    fn resolve_url(&self, thumbnail_ref: &ThumbnailRef) -> Result<String> {
        Ok(format!(
            "thumbnail://{}/{}/{}",
            thumbnail_ref.package_id, thumbnail_ref.variant, thumbnail_ref.content_hash
        ))
    }
}

fn sanitize_path_segment(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "unknown".to_owned()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::ThumbnailStore;

    #[test]
    fn stores_thumbnail_and_returns_opaque_url() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = FileSystemThumbnailStore::new(temp.path().to_path_buf());

        let thumbnail_ref = store
            .put_thumbnail("pkg-1", "abcdef", "jpg", b"thumbnail bytes")
            .expect("put thumbnail");
        let url = store
            .resolve_url(&thumbnail_ref)
            .expect("resolve thumbnail url");

        assert_eq!(thumbnail_ref.package_id, "pkg-1");
        assert_eq!(thumbnail_ref.content_hash, "abcdef");
        assert_eq!(thumbnail_ref.variant, "preview-768");
        assert_eq!(url, "thumbnail://pkg-1/preview-768/abcdef");
        assert!(!url.contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn concurrent_same_key_writes_do_not_collide_on_temp_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = std::sync::Arc::new(FileSystemThumbnailStore::new(temp.path().to_path_buf()));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let store = store.clone();
            handles.push(std::thread::spawn(move || {
                store.put_thumbnail("pkg-1", "abcdef", "jpg", b"thumbnail bytes")
            }));
        }

        for handle in handles {
            handle
                .join()
                .expect("writer thread should not panic")
                .expect("put thumbnail should not fail");
        }

        let final_path = temp
            .path()
            .join("thumbnails")
            .join("pkg-1")
            .join("preview-768-abcdef.jpg");
        assert_eq!(
            std::fs::read(final_path).expect("read final thumbnail"),
            b"thumbnail bytes"
        );
    }
}
