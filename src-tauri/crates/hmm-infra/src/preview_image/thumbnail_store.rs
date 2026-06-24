use anyhow::Result;
use hmm_ports::{
    ThumbnailCacheMaintenance, ThumbnailCacheMaintenanceRequest, ThumbnailRef, ThumbnailStore,
};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub struct FileSystemThumbnailStore {
    root_dir: PathBuf,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ThumbnailPruneReport {
    pub deleted_files: usize,
    pub deleted_package_dirs: usize,
    pub skipped_entries: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ThumbnailSizePruneReport {
    pub deleted_files: usize,
    pub deleted_package_dirs: usize,
    pub skipped_entries: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ThumbnailCacheKey {
    package_id: String,
    content_hash: String,
    variant: String,
}

struct ThumbnailCacheFile {
    path: PathBuf,
    package_dir: PathBuf,
    file_name: String,
    size_bytes: u64,
    lru_time: SystemTime,
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
        let retained = retained_thumbnail_keys(retained);
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

    pub fn prune_to_size_limit<I, R>(
        &self,
        max_bytes: u64,
        retained: I,
    ) -> Result<ThumbnailSizePruneReport>
    where
        I: IntoIterator<Item = R>,
        R: Borrow<ThumbnailRef>,
    {
        let retained = retained_thumbnail_keys(retained);
        let thumbnails_dir = self.root_dir.join("thumbnails");
        let mut report = ThumbnailSizePruneReport::default();

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
        let mut candidates = Vec::new();
        let mut package_dirs = Vec::new();

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
            package_dirs.push(package_path.clone());

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

                let canonical_thumbnail_path = fs::canonicalize(&thumbnail_path)?;
                if !canonical_thumbnail_path.starts_with(&canonical_thumbnails_dir) {
                    report.skipped_entries += 1;
                    continue;
                }

                let size_bytes = thumbnail_metadata.len();
                report.bytes_before = report.bytes_before.saturating_add(size_bytes);
                if is_retained_thumbnail_file(&retained, &package_id, &file_name) {
                    continue;
                }

                candidates.push(ThumbnailCacheFile {
                    path: thumbnail_path,
                    package_dir: package_path.clone(),
                    file_name,
                    size_bytes,
                    lru_time: thumbnail_metadata
                        .accessed()
                        .or_else(|_| thumbnail_metadata.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH),
                });
            }
        }

        let mut bytes_after = report.bytes_before;
        candidates.sort_by(|left, right| {
            left.lru_time
                .cmp(&right.lru_time)
                .then_with(|| left.package_dir.cmp(&right.package_dir))
                .then_with(|| left.file_name.cmp(&right.file_name))
        });

        for candidate in candidates {
            if bytes_after <= max_bytes {
                break;
            }

            if !path_stays_inside(&canonical_thumbnails_dir, &candidate.path)? {
                report.skipped_entries += 1;
                continue;
            }

            fs::remove_file(&candidate.path)?;
            report.deleted_files += 1;
            bytes_after = bytes_after.saturating_sub(candidate.size_bytes);
        }

        for package_dir in package_dirs {
            match fs::remove_dir(&package_dir) {
                Ok(()) => report.deleted_package_dirs += 1,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }

        report.bytes_after = bytes_after;
        Ok(report)
    }

    pub fn prune_unreferenced_thumbnails_older_than<I, R>(
        &self,
        max_age: Duration,
        retained: I,
    ) -> Result<ThumbnailPruneReport>
    where
        I: IntoIterator<Item = R>,
        R: Borrow<ThumbnailRef>,
    {
        let retained = retained_thumbnail_keys(retained);
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
        let now = SystemTime::now();

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

                let lru_time = thumbnail_metadata
                    .accessed()
                    .or_else(|_| thumbnail_metadata.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                if now
                    .duration_since(lru_time)
                    .map(|age| age < max_age)
                    .unwrap_or(true)
                {
                    continue;
                }

                if !path_stays_inside(&canonical_thumbnails_dir, &thumbnail_path)? {
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
        variant: &str,
        extension: &str,
        bytes: &[u8],
    ) -> Result<ThumbnailRef> {
        let safe_package_id = sanitize_path_segment(package_id);
        let safe_hash = sanitize_path_segment(content_hash);
        let variant = sanitize_path_segment(variant);
        let safe_extension = sanitize_path_segment(extension);
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

impl ThumbnailCacheMaintenance for FileSystemThumbnailStore {
    fn maintain_thumbnail_cache(
        &self,
        request: ThumbnailCacheMaintenanceRequest<'_>,
    ) -> Result<()> {
        if let Some(max_age) = request.max_age {
            FileSystemThumbnailStore::prune_unreferenced_thumbnails_older_than(
                self,
                max_age,
                request.retained,
            )?;
        } else {
            FileSystemThumbnailStore::prune_unreferenced_thumbnails(self, request.retained)?;
        }
        if let Some(max_bytes) = request.max_bytes {
            FileSystemThumbnailStore::prune_to_size_limit(self, max_bytes, request.retained)?;
        }
        Ok(())
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

fn retained_thumbnail_keys<I, R>(retained: I) -> HashSet<ThumbnailCacheKey>
where
    I: IntoIterator<Item = R>,
    R: Borrow<ThumbnailRef>,
{
    retained
        .into_iter()
        .map(|thumbnail_ref| {
            let thumbnail_ref = thumbnail_ref.borrow();
            ThumbnailCacheKey {
                package_id: sanitize_path_segment(&thumbnail_ref.package_id),
                content_hash: sanitize_path_segment(&thumbnail_ref.content_hash),
                variant: sanitize_path_segment(&thumbnail_ref.variant),
            }
        })
        .collect()
}

fn path_stays_inside(root: &Path, path: &Path) -> Result<bool> {
    Ok(fs::canonicalize(path)?.starts_with(root))
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
            .put_thumbnail("pkg-1", "abcdef", "preview-768", "jpg", b"thumbnail bytes")
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
    fn stores_thumbnail_with_caller_selected_variant() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = FileSystemThumbnailStore::new(temp.path().to_path_buf());

        let thumbnail_ref = store
            .put_thumbnail("pkg-1", "abcdef", "preview-1024", "jpg", b"thumbnail bytes")
            .expect("put thumbnail");
        let url = store
            .resolve_url(&thumbnail_ref)
            .expect("resolve thumbnail url");

        assert_eq!(thumbnail_ref.variant, "preview-1024");
        assert_eq!(url, "thumbnail://pkg-1/preview-1024/abcdef");
        assert!(temp
            .path()
            .join("thumbnails")
            .join("pkg-1")
            .join("preview-1024-abcdef.jpg")
            .exists());
    }

    #[test]
    fn concurrent_same_key_writes_do_not_collide_on_temp_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = std::sync::Arc::new(FileSystemThumbnailStore::new(temp.path().to_path_buf()));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let store = store.clone();
            handles.push(std::thread::spawn(move || {
                store.put_thumbnail("pkg-1", "abcdef", "preview-768", "jpg", b"thumbnail bytes")
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
            .put_thumbnail("pkg-1", "keep", "preview-768", "jpg", b"keep")
            .expect("put retained thumbnail");
        store
            .put_thumbnail("pkg-1", "stale", "preview-768", "jpg", b"stale")
            .expect("put stale thumbnail");
        store
            .put_thumbnail("pkg-2", "old", "preview-768", "jpg", b"old")
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

    #[test]
    fn prunes_unretained_thumbnails_by_lru_until_size_limit_is_met() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = FileSystemThumbnailStore::new(temp.path().to_path_buf());

        let retained_old = store
            .put_thumbnail("pkg-1", "retained-old", "preview-768", "jpg", b"1111")
            .expect("put retained old thumbnail");
        std::thread::sleep(std::time::Duration::from_millis(20));
        store
            .put_thumbnail("pkg-1", "delete-old", "preview-768", "jpg", b"2222")
            .expect("put deletable old thumbnail");
        std::thread::sleep(std::time::Duration::from_millis(20));
        store
            .put_thumbnail("pkg-1", "keep-new", "preview-768", "jpg", b"3333")
            .expect("put newer thumbnail");

        let report = store
            .prune_to_size_limit(8, std::slice::from_ref(&retained_old))
            .expect("prune thumbnails to size limit");

        assert_eq!(report.deleted_files, 1);
        assert_eq!(report.bytes_before, 12);
        assert_eq!(report.bytes_after, 8);
        assert!(thumbnail_path(temp.path(), "pkg-1", "retained-old").exists());
        assert!(!thumbnail_path(temp.path(), "pkg-1", "delete-old").exists());
        assert!(thumbnail_path(temp.path(), "pkg-1", "keep-new").exists());
    }

    #[test]
    fn prunes_only_expired_unreferenced_thumbnails_when_age_limit_is_set() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = FileSystemThumbnailStore::new(temp.path().to_path_buf());

        let retained = store
            .put_thumbnail("pkg-1", "retained", "preview-768", "jpg", b"retained")
            .expect("put retained thumbnail");
        store
            .put_thumbnail("pkg-1", "expired", "preview-768", "jpg", b"expired")
            .expect("put expired thumbnail");
        std::thread::sleep(std::time::Duration::from_millis(100));
        store
            .put_thumbnail("pkg-1", "young", "preview-768", "jpg", b"young")
            .expect("put young thumbnail");

        let report = store
            .prune_unreferenced_thumbnails_older_than(
                std::time::Duration::from_millis(50),
                std::slice::from_ref(&retained),
            )
            .expect("prune thumbnails by age");

        assert_eq!(report.deleted_files, 1);
        assert!(thumbnail_path(temp.path(), "pkg-1", "retained").exists());
        assert!(!thumbnail_path(temp.path(), "pkg-1", "expired").exists());
        assert!(thumbnail_path(temp.path(), "pkg-1", "young").exists());
    }

    #[test]
    fn size_prune_skips_symlinked_cache_entries_without_following_them() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside temp dir");
        let store = FileSystemThumbnailStore::new(temp.path().to_path_buf());
        store
            .put_thumbnail("pkg-1", "delete", "preview-768", "jpg", b"delete")
            .expect("put thumbnail");
        let package_dir = temp.path().join("thumbnails").join("pkg-1");
        let outside_file = outside.path().join("outside.jpg");
        std::fs::write(&outside_file, b"outside").expect("write outside target");
        let link_path = package_dir.join("preview-768-link.jpg");

        if !try_create_symlink(&outside_file, &link_path) {
            return;
        }

        let report = store
            .prune_to_size_limit(0, std::iter::empty::<&ThumbnailRef>())
            .expect("prune thumbnails to size limit");

        assert_eq!(report.deleted_files, 1);
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
