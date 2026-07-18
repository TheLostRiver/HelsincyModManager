use crate::dto::CommandErrorDto;
use crate::mod_library_dto::{
    ModLibraryFilterDto, ModLibraryPageDto, ModLibraryProfileContextDto, QueryModLibraryRequestDto,
};
use crate::state::AppState;
use hmm_app::{
    InstallManifestStatus, ModLibraryFilter, ModLibraryProfileContext, ModLibraryQuery,
    ModLibraryQueryError, ModLibraryQueryService, ModLibrarySort,
};
use hmm_core::{GameId, ProfileId};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn query_mod_library(
    request: QueryModLibraryRequestDto,
    state: State<'_, AppState>,
) -> Result<ModLibraryPageDto, CommandErrorDto> {
    let query = mod_library_query_from_dto(request)?;
    ModLibraryQueryService::new(
        Arc::clone(&state.mod_library),
        state.install_manifest_query.clone(),
    )
    .query(query)
    .map(Into::into)
    .map_err(mod_library_query_error_to_command_error)
}

fn mod_library_query_from_dto(
    request: QueryModLibraryRequestDto,
) -> Result<ModLibraryQuery, CommandErrorDto> {
    let page = u64::try_from(request.page)
        .ok()
        .filter(|page| *page > 0)
        .ok_or_else(mod_library_page_invalid_error)?;
    let page_size =
        u32::try_from(request.page_size).map_err(|_| mod_library_page_size_unsupported_error())?;

    Ok(ModLibraryQuery {
        profile_context: request
            .profile_context
            .map(mod_library_profile_context_from_dto)
            .transpose()?,
        search: request.search,
        filter: mod_library_filter_from_dto(request.filter)?,
        sort: mod_library_sort_from_dto(&request.sort)?,
        page,
        page_size,
    })
}

fn mod_library_profile_context_from_dto(
    context: ModLibraryProfileContextDto,
) -> Result<ModLibraryProfileContext, CommandErrorDto> {
    let game_id = GameId::parse(context.game_id).map_err(|_| CommandErrorDto {
        code: "game_id_invalid".to_owned(),
        message: "game id is invalid".to_owned(),
    })?;
    let profile_id = context.profile_id.trim();
    if profile_id.is_empty() {
        return Err(CommandErrorDto {
            code: "profile_id_empty".to_owned(),
            message: "profile id cannot be empty".to_owned(),
        });
    }

    Ok(ModLibraryProfileContext {
        game_id,
        profile_id: ProfileId::new(profile_id),
    })
}

fn mod_library_filter_from_dto(
    filter: ModLibraryFilterDto,
) -> Result<ModLibraryFilter, CommandErrorDto> {
    match filter.kind.as_str() {
        "all" if filter.status.is_none() && filter.category_id.is_none() => {
            Ok(ModLibraryFilter::All)
        }
        "status" if filter.category_id.is_none() => {
            let status = filter
                .status
                .as_deref()
                .and_then(parse_install_manifest_status)
                .ok_or_else(mod_library_filter_invalid_error)?;
            Ok(ModLibraryFilter::Status(status))
        }
        "category" if filter.status.is_none() => {
            let category_id = filter
                .category_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(mod_library_filter_invalid_error)?;
            Ok(ModLibraryFilter::Category(category_id.to_owned()))
        }
        _ => Err(mod_library_filter_invalid_error()),
    }
}

fn parse_install_manifest_status(value: &str) -> Option<InstallManifestStatus> {
    match value {
        "not_installed" => Some(InstallManifestStatus::NotInstalled),
        "installed" => Some(InstallManifestStatus::Installed),
        "committed_cleanup_pending" => Some(InstallManifestStatus::CommittedCleanupPending),
        "cleanup_pending" => Some(InstallManifestStatus::CleanupPending),
        "rollback_required" => Some(InstallManifestStatus::RollbackRequired),
        "repair_required" => Some(InstallManifestStatus::RepairRequired),
        "unknown" => Some(InstallManifestStatus::Unknown),
        _ => None,
    }
}

fn mod_library_sort_from_dto(value: &str) -> Result<ModLibrarySort, CommandErrorDto> {
    match value {
        "name_asc" => Ok(ModLibrarySort::NameAsc),
        _ => Err(CommandErrorDto {
            code: "mod_library_sort_invalid".to_owned(),
            message: "mod library sort is invalid".to_owned(),
        }),
    }
}

fn mod_library_query_error_to_command_error(error: ModLibraryQueryError) -> CommandErrorDto {
    match error {
        ModLibraryQueryError::PageInvalid => mod_library_page_invalid_error(),
        ModLibraryQueryError::PageSizeUnsupported => mod_library_page_size_unsupported_error(),
        ModLibraryQueryError::SearchTooLong => CommandErrorDto {
            code: "mod_library_search_too_long".to_owned(),
            message: "mod library search is too long".to_owned(),
        },
        ModLibraryQueryError::CategoryNotFound => CommandErrorDto {
            code: "mod_library_category_not_found".to_owned(),
            message: "mod library category was not found".to_owned(),
        },
        ModLibraryQueryError::ProfileContextRequired => CommandErrorDto {
            code: "mod_library_profile_context_required".to_owned(),
            message: "mod library profile context is required".to_owned(),
        },
        ModLibraryQueryError::LibraryUnavailable => CommandErrorDto {
            code: "mod_library_unavailable".to_owned(),
            message: "mod library is unavailable".to_owned(),
        },
        ModLibraryQueryError::StatusUnavailable => CommandErrorDto {
            code: "mod_library_status_unavailable".to_owned(),
            message: "mod library install status is unavailable".to_owned(),
        },
    }
}

fn mod_library_page_invalid_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "mod_library_page_invalid".to_owned(),
        message: "mod library page must start at one".to_owned(),
    }
}

fn mod_library_page_size_unsupported_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "mod_library_page_size_unsupported".to_owned(),
        message: "mod library page size is unsupported".to_owned(),
    }
}

fn mod_library_filter_invalid_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "mod_library_filter_invalid".to_owned(),
        message: "mod library filter is invalid".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(filter: ModLibraryFilterDto) -> QueryModLibraryRequestDto {
        QueryModLibraryRequestDto {
            profile_context: None,
            search: String::new(),
            filter,
            sort: "name_asc".to_owned(),
            page: 1,
            page_size: 24,
        }
    }

    fn all_filter() -> ModLibraryFilterDto {
        ModLibraryFilterDto {
            kind: "all".to_owned(),
            status: None,
            category_id: None,
        }
    }

    #[test]
    fn request_maps_profile_filter_sort_and_page() {
        let query = mod_library_query_from_dto(QueryModLibraryRequestDto {
            profile_context: Some(ModLibraryProfileContextDto {
                game_id: "mhw".to_owned(),
                profile_id: "  default  ".to_owned(),
            }),
            search: "fatalis".to_owned(),
            filter: ModLibraryFilterDto {
                kind: "status".to_owned(),
                status: Some("installed".to_owned()),
                category_id: None,
            },
            sort: "name_asc".to_owned(),
            page: 2,
            page_size: 48,
        })
        .expect("map valid mod library query");

        let context = query.profile_context.expect("profile context");
        assert_eq!(context.game_id.as_str(), "mhw");
        assert_eq!(context.profile_id.as_str(), "default");
        assert_eq!(query.search, "fatalis");
        assert_eq!(
            query.filter,
            ModLibraryFilter::Status(InstallManifestStatus::Installed)
        );
        assert_eq!(query.sort, ModLibrarySort::NameAsc);
        assert_eq!(query.page, 2);
        assert_eq!(query.page_size, 48);
    }

    #[test]
    fn request_rejects_invalid_filter_shapes_with_stable_error() {
        for filter in [
            ModLibraryFilterDto {
                kind: "unknown".to_owned(),
                status: None,
                category_id: None,
            },
            ModLibraryFilterDto {
                kind: "status".to_owned(),
                status: Some("disabled".to_owned()),
                category_id: None,
            },
            ModLibraryFilterDto {
                kind: "category".to_owned(),
                status: None,
                category_id: Some("  ".to_owned()),
            },
            ModLibraryFilterDto {
                kind: "all".to_owned(),
                status: Some("installed".to_owned()),
                category_id: None,
            },
        ] {
            let error = mod_library_query_from_dto(request(filter)).expect_err("invalid filter");
            assert_eq!(error.code, "mod_library_filter_invalid");
            assert!(!error.message.contains(':'));
            assert!(!error.message.contains('\\'));
        }
    }

    #[test]
    fn request_rejects_invalid_page_sort_and_profile_context() {
        let mut invalid_page = request(all_filter());
        invalid_page.page = 0;
        assert_eq!(
            mod_library_query_from_dto(invalid_page)
                .expect_err("zero page rejected")
                .code,
            "mod_library_page_invalid"
        );

        let mut invalid_page_size = request(all_filter());
        invalid_page_size.page_size = -1;
        assert_eq!(
            mod_library_query_from_dto(invalid_page_size)
                .expect_err("negative page size rejected")
                .code,
            "mod_library_page_size_unsupported"
        );

        let mut invalid_sort = request(all_filter());
        invalid_sort.sort = "imported_desc".to_owned();
        assert_eq!(
            mod_library_query_from_dto(invalid_sort)
                .expect_err("unknown sort rejected")
                .code,
            "mod_library_sort_invalid"
        );

        let mut invalid_profile = request(all_filter());
        invalid_profile.profile_context = Some(ModLibraryProfileContextDto {
            game_id: "mhw".to_owned(),
            profile_id: "  ".to_owned(),
        });
        assert_eq!(
            mod_library_query_from_dto(invalid_profile)
                .expect_err("empty profile id rejected")
                .code,
            "profile_id_empty"
        );
    }

    #[test]
    fn app_errors_map_to_stable_path_free_codes() {
        let cases = [
            (
                ModLibraryQueryError::PageInvalid,
                "mod_library_page_invalid",
            ),
            (
                ModLibraryQueryError::PageSizeUnsupported,
                "mod_library_page_size_unsupported",
            ),
            (
                ModLibraryQueryError::SearchTooLong,
                "mod_library_search_too_long",
            ),
            (
                ModLibraryQueryError::CategoryNotFound,
                "mod_library_category_not_found",
            ),
            (
                ModLibraryQueryError::ProfileContextRequired,
                "mod_library_profile_context_required",
            ),
            (
                ModLibraryQueryError::LibraryUnavailable,
                "mod_library_unavailable",
            ),
            (
                ModLibraryQueryError::StatusUnavailable,
                "mod_library_status_unavailable",
            ),
        ];

        for (error, expected_code) in cases {
            let error = mod_library_query_error_to_command_error(error);
            assert_eq!(error.code, expected_code);
            assert!(!error.message.contains(':'));
            assert!(!error.message.contains('\\'));
        }
    }

    #[test]
    fn command_source_stays_thin_and_registered() {
        let source = include_str!("mod_library_commands.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production command source");
        let registration = include_str!("lib.rs");

        assert!(source.contains("ModLibraryQueryService::new"));
        assert!(source.contains("state.mod_library"));
        assert!(source.contains("state.install_manifest_query"));
        assert!(source.contains(".query(query)"));
        assert!(registration.contains("mod_library_commands::query_mod_library"));
        assert!(registration.contains("query_mod_library,"));

        for forbidden in [
            "std::fs",
            "PathBuf",
            "install_recovery_scanner",
            "manifest_repository",
            "sandbox",
        ] {
            assert!(
                !source.contains(forbidden),
                "command source contains {forbidden}"
            );
        }
    }
}
