use anyhow::Result;
use hmm_ports::{ThumbnailRef, ThumbnailStore};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub struct FileSystemThumbnailStore {
    root_dir: PathBuf,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ThumbnailPruneReport {
    pub deleted_files: usize,
    pub deleted_package_dirs: usize,
    pub skipped_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ThumbnailCacheKey {
    package_id: String,
    content_hash: String,
    variant: String,
}

impl FileSystemThumbnailStore {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    pub fn prune_unreferenced_thumbnails<I, R>(&self, retained: I) -> Result<ThumbnailPruneReport>
    where
        I: IntoIterator<Item = R>,
        R: Borrow<ThumbnailRef>,
    {
        let retained = retained
            .into_iter()
            .map(|thumbnail_ref| {
                let thumbnail_ref = thumbnail_ref.borrow();
                ThumbnailCacheKey {
                    package_id: sanitize_path_segment(&thumbnail_ref.package_id),
                    content_hash: sanitize_path_segment(&thumbnail_ref.content_hash),
                    variant: sanitize_path_segment(&thumbnail_ref.variant),
                }
            })
            .collect::<HashSet<_>>();
        let thumbnails_dir = self.root_dir.join("thumbnails");
        let mut report = ThumbnailPruneReport::default();

        let thumbnails_metadata = match fs::symlink_metadata(&thumbnails_dir) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(report),
            Err(error) => return Err(error.into()),
        };

        if thumbnails_metadata.file_type().is_symlink() || !thumbnails_metadata.is_dir() {
            report.skipped_entries += 1;
            return Ok(report);
        }

        let canonical_thumbnails_dir = fs::canonicalize(&thumbnails_dir)?;

        for package_entry in fs::read_dir(&thumbnails_dir)? {
            let package_entry = package_entry?;
            let package_path = package_entry.path();
            let package_metadata = fs::symlink_metadata(&package_path)?;
            if package_metadata.file_type().is_symlink() || !package_metadata.is_dir() {
                report.skipped_entries += 1;
                continue;
            }

            let Some(package_id) = package_entry.file_name().to_str().map(str::to_owned) else {
                report.skipped_entries += 1;
                continue;
            };

            let canonical_package_dir = fs::canonicalize(&package_path)?;
            if !canonical_package_dir.starts_with(&canonical_thumbnails_dir) {
                report.skipped_entries += 1;
                continue;
            }

            for thumbnail_entry in fs::read_dir(&package_path)? {
                let thumbnail_entry = thumbnail_entry?;
                let thumbnail_path = thumbnail_entry.path();
                let thumbnail_metadata = fs::symlink_metadata(&thumbnail_path)?;
                if thumbnail_metadata.file_type().is_symlink() || !thumbnail_metadata.is_file() {
                    report.skipped_entries += 1;
                    continue;
                }

                let Some(file_name) = thumbnail_entry.file_name().to_str().map(str::to_owned)
                else {
                    report.skipped_entries += 1;
                    continue;
                };

                if is_retained_thumbnail_file(&retained, &package_id, &file_name) {
                    continue;
                }

                let canonical_thumbnail_path = fs::canonicalize(&thumbnail_path)?;
                if !canonical_thumbnail_path.starts_with(&canonical_thumbnails_dir) {
                    report.skipped_entries += 1;
                    continue;
                }

                fs::remove_file(&thumbnail_path)?;
                report.deleted_files += 1;
            }

            match fs::remove_dir(&package_path) {
                Ok(()) => report.deleted_package_dirs += 1,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }

        Ok(report)
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

fn is_retained_thumbnail_file(
    retained: &HashSet<ThumbnailCacheKey>,
    package_id: &str,
    file_name: &str,
) -> bool {
    retained.iter().any(|thumbnail_ref| {
        thumbnail_ref.package_id == package_id
            && file_name.starts_with(&format!(
                "{}-{}.",
                thumbnail_ref.variant, thumbnail_ref.content_hash
            ))
    })
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

    #[test]
    fn prunes_unreferenced_thumbnails_and_keeps_retained_refs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = FileSystemThumbnailStore::new(temp.path().to_path_buf());

        let retained = store
            .put_thumbnail("pkg-1", "keep", "jpg", b"keep")
            .expect("put retained thumbnail");
        store
            .put_thumbnail("pkg-1", "stale", "jpg", b"stale")
            .expect("put stale thumbnail");
        store
            .put_thumbnail("pkg-2", "old", "jpg", b"old")
            .expect("put stale package thumbnail");

        let report = store
            .prune_unreferenced_thumbnails(std::slice::from_ref(&retained))
            .expect("prune thumbnails");

        assert_eq!(report.deleted_files, 2);
        assert_eq!(report.deleted_package_dirs, 1);
        assert!(thumbnail_path(temp.path(), "pkg-1", "keep").exists());
        assert!(!thumbnail_path(temp.path(), "pkg-1", "stale").exists());
        assert!(!temp.path().join("thumbnails").join("pkg-2").exists());
    }

    #[test]
    fn prune_skips_symlinked_cache_entries_without_following_them() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside temp dir");
        let store = FileSystemThumbnailStore::new(temp.path().to_path_buf());
        let package_dir = temp.path().join("thumbnails").join("pkg-1");
        std::fs::create_dir_all(&package_dir).expect("create package dir");
        let outside_file = outside.path().join("outside.jpg");
        std::fs::write(&outside_file, b"outside").expect("write outside target");
        let link_path = package_dir.join("preview-768-stale.jpg");

        if !try_create_symlink(&outside_file, &link_path) {
            return;
        }

        let report = store
            .prune_unreferenced_thumbnails(std::iter::empty::<&ThumbnailRef>())
            .expect("prune thumbnails");

        assert_eq!(report.deleted_files, 0);
        assert_eq!(report.skipped_entries, 1);
        assert!(outside_file.exists());
        assert!(link_path.exists());
    }

    fn thumbnail_path(root: &std::path::Path, package_id: &str, content_hash: &str) -> PathBuf {
        root.join("thumbnails")
            .join(package_id)
            .join(format!("preview-768-{content_hash}.jpg"))
    }

    #[cfg(unix)]
    fn try_create_symlink(target: &std::path::Path, link: &std::path::Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn try_create_symlink(target: &std::path::Path, link: &std::path::Path) -> bool {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
}
