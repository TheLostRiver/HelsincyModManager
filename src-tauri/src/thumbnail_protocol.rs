use std::path::Path;
use tauri::{http, Manager, Runtime};

const THUMBNAIL_SCHEME: &str = "thumbnail://";
const WINDOWS_THUMBNAIL_ORIGIN: &str = "http://thumbnail.localhost/";
const CACHE_EXTENSIONS: [(&str, &str); 4] = [
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("webp", "image/webp"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThumbnailReference {
    package_id: String,
    variant: String,
    content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThumbnailResponse {
    bytes: Vec<u8>,
    content_type: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThumbnailProtocolError {
    BadRequest,
    NotFound,
    Internal,
}

pub fn register_thumbnail_protocol<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_uri_scheme_protocol("thumbnail", |context, request| {
        let app_data_dir = match context.app_handle().path().app_data_dir() {
            Ok(path) => path,
            Err(_) => return empty_response(http::StatusCode::INTERNAL_SERVER_ERROR),
        };

        match read_thumbnail(&app_data_dir, &request.uri().to_string()) {
            Ok(thumbnail) => http::Response::builder()
                .status(http::StatusCode::OK)
                .header(http::header::CONTENT_TYPE, thumbnail.content_type)
                .header(
                    http::header::CACHE_CONTROL,
                    "public, max-age=31536000, immutable",
                )
                .body(thumbnail.bytes)
                .expect("thumbnail response should be valid"),
            Err(ThumbnailProtocolError::BadRequest) => {
                empty_response(http::StatusCode::BAD_REQUEST)
            }
            Err(ThumbnailProtocolError::NotFound) => empty_response(http::StatusCode::NOT_FOUND),
            Err(ThumbnailProtocolError::Internal) => {
                empty_response(http::StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    })
}

fn empty_response(status: http::StatusCode) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Vec::new())
        .expect("empty thumbnail response should be valid")
}

fn parse_thumbnail_url(uri: &str) -> Result<ThumbnailReference, ThumbnailProtocolError> {
    let path = uri
        .strip_prefix(THUMBNAIL_SCHEME)
        .or_else(|| uri.strip_prefix(WINDOWS_THUMBNAIL_ORIGIN))
        .ok_or(ThumbnailProtocolError::BadRequest)?;
    let mut segments = path.split('/');
    let package_id = segments.next().ok_or(ThumbnailProtocolError::BadRequest)?;
    let variant = segments.next().ok_or(ThumbnailProtocolError::BadRequest)?;
    let content_hash = segments.next().ok_or(ThumbnailProtocolError::BadRequest)?;

    if segments.next().is_some()
        || !is_safe_segment(package_id)
        || !is_safe_segment(variant)
        || !is_safe_segment(content_hash)
    {
        return Err(ThumbnailProtocolError::BadRequest);
    }

    Ok(ThumbnailReference {
        package_id: package_id.to_owned(),
        variant: variant.to_owned(),
        content_hash: content_hash.to_owned(),
    })
}

fn read_thumbnail(
    cache_root: &Path,
    uri: &str,
) -> Result<ThumbnailResponse, ThumbnailProtocolError> {
    let reference = parse_thumbnail_url(uri)?;
    let thumbnails_root = cache_root.join("thumbnails");
    if !is_registered_cache_dir(&thumbnails_root) {
        return Err(ThumbnailProtocolError::BadRequest);
    }

    let package_dir = thumbnails_root.join(&reference.package_id);
    if !is_registered_cache_dir(&package_dir) {
        return Err(ThumbnailProtocolError::BadRequest);
    }

    for (extension, content_type) in CACHE_EXTENSIONS {
        let candidate = package_dir.join(format!(
            "{}-{}.{}",
            reference.variant, reference.content_hash, extension
        ));
        let metadata = match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(ThumbnailProtocolError::Internal),
        };
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            return Err(ThumbnailProtocolError::BadRequest);
        }
        if !file_type.is_file() {
            continue;
        }

        if !is_inside_root(&thumbnails_root, &candidate) {
            return Err(ThumbnailProtocolError::BadRequest);
        }

        let bytes = std::fs::read(candidate).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ThumbnailProtocolError::NotFound
            } else {
                ThumbnailProtocolError::Internal
            }
        })?;
        return Ok(ThumbnailResponse {
            bytes,
            content_type,
        });
    }

    Err(ThumbnailProtocolError::NotFound)
}

fn is_registered_cache_dir(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    let file_type = metadata.file_type();
    file_type.is_dir() && !file_type.is_symlink()
}

fn is_safe_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn is_inside_root(root: &Path, candidate: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(candidate) = candidate.canonicalize() else {
        return false;
    };
    candidate.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_valid_thumbnail_url() {
        let reference = parse_thumbnail_url("thumbnail://pkg-1/preview-768/abcdef0123456789")
            .expect("valid thumbnail url");

        assert_eq!(reference.package_id, "pkg-1");
        assert_eq!(reference.variant, "preview-768");
        assert_eq!(reference.content_hash, "abcdef0123456789");
    }

    #[test]
    fn parses_windows_localhost_thumbnail_url() {
        let reference = parse_thumbnail_url("http://thumbnail.localhost/pkg-1/preview-768/abcdef")
            .expect("valid thumbnail url");

        assert_eq!(reference.package_id, "pkg-1");
        assert_eq!(reference.variant, "preview-768");
        assert_eq!(reference.content_hash, "abcdef");
    }

    #[test]
    fn rejects_path_traversal_segments() {
        assert_eq!(
            parse_thumbnail_url("thumbnail://pkg-1/../abcdef").expect_err("invalid url"),
            ThumbnailProtocolError::BadRequest
        );
        assert_eq!(
            parse_thumbnail_url("thumbnail://pkg-1/preview-768/..%2fsecret")
                .expect_err("invalid url"),
            ThumbnailProtocolError::BadRequest
        );
    }

    #[test]
    fn reads_existing_thumbnail_without_exposing_root_path() {
        let root = temp_root("thumbnail-protocol-read");
        let thumbnail_dir = root.join("thumbnails").join("pkg-1");
        fs::create_dir_all(&thumbnail_dir).expect("create thumbnail dir");
        fs::write(
            thumbnail_dir.join("preview-768-abcdef.jpg"),
            b"thumbnail bytes",
        )
        .expect("write thumbnail");

        let response =
            read_thumbnail(&root, "thumbnail://pkg-1/preview-768/abcdef").expect("read thumbnail");

        assert_eq!(response.content_type, "image/jpeg");
        assert_eq!(response.bytes, b"thumbnail bytes");
        assert!(!response
            .content_type
            .contains(root.to_string_lossy().as_ref()));

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn returns_not_found_for_missing_thumbnail() {
        let root = temp_root("thumbnail-protocol-missing");
        fs::create_dir_all(root.join("thumbnails").join("pkg-1"))
            .expect("create package thumbnail dir");

        assert_eq!(
            read_thumbnail(&root, "thumbnail://pkg-1/preview-768/missing")
                .expect_err("missing thumbnail"),
            ThumbnailProtocolError::NotFound
        );

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn rejects_unregistered_package_directory() {
        let root = temp_root("thumbnail-protocol-unregistered-package");
        fs::create_dir_all(root.join("thumbnails")).expect("create thumbnails dir");

        assert_eq!(
            read_thumbnail(&root, "thumbnail://pkg-1/preview-768/abcdef")
                .expect_err("unregistered package rejected"),
            ThumbnailProtocolError::BadRequest
        );

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn rejects_package_reference_that_is_not_directory() {
        let root = temp_root("thumbnail-protocol-package-file");
        let thumbnails_root = root.join("thumbnails");
        fs::create_dir_all(&thumbnails_root).expect("create thumbnails dir");
        fs::write(thumbnails_root.join("pkg-1"), b"not a package dir").expect("write package file");

        assert_eq!(
            read_thumbnail(&root, "thumbnail://pkg-1/preview-768/abcdef")
                .expect_err("package file rejected"),
            ThumbnailProtocolError::BadRequest
        );

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn rejects_symlinked_thumbnails_root_without_following_it() {
        let root = temp_root("thumbnail-protocol-symlink-root");
        let outside = temp_root("thumbnail-protocol-outside");
        let thumbnails_link = root.join("thumbnails");
        let outside_package = outside.join("pkg-1");
        fs::create_dir_all(&outside_package).expect("create outside package");
        fs::write(
            outside_package.join("preview-768-abcdef.jpg"),
            b"outside bytes",
        )
        .expect("write outside thumbnail");
        fs::create_dir_all(&root).expect("create app data root");

        if !try_create_dir_symlink(&outside, &thumbnails_link) {
            fs::remove_dir_all(root).expect("cleanup temp root");
            fs::remove_dir_all(outside).expect("cleanup outside root");
            return;
        }

        assert_eq!(
            read_thumbnail(&root, "thumbnail://pkg-1/preview-768/abcdef")
                .expect_err("symlinked thumbnails root rejected"),
            ThumbnailProtocolError::BadRequest
        );

        let _ = fs::remove_dir(&thumbnails_link);
        fs::remove_dir_all(root).expect("cleanup temp root");
        fs::remove_dir_all(outside).expect("cleanup outside root");
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{suffix}"))
    }

    #[cfg(unix)]
    fn try_create_dir_symlink(target: &std::path::Path, link: &std::path::Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn try_create_dir_symlink(target: &std::path::Path, link: &std::path::Path) -> bool {
        if std::os::windows::fs::symlink_dir(target, link).is_ok() {
            return true;
        }

        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output();
        output.is_ok_and(|output| output.status.success())
    }
}
