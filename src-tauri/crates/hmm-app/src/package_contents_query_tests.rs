use super::*;
use anyhow::Result;
use hmm_core::{GameDirectoryValidation, PreviewImageRejectionReason};
use hmm_ports::{
    GameDirectoryProbe, GamePrerequisiteReport, ModPackageContentEntry,
    ModPackageContentRootRepository, ModPackageContents, StoredImportPreviewImage,
    StoredModImportAnalysis, StoredModPackageMetadata,
};
use std::path::PathBuf;

struct StubAdapter {
    allowed_roots: Vec<String>,
    rejected_extensions: Vec<String>,
}

impl GameAdapter for StubAdapter {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn display_name(&self) -> &'static str {
        "stub"
    }

    fn validate_directory(&self, _probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation {
        unreachable!("package contents query never validates a game directory")
    }

    fn inspect_prerequisites(&self, _probe: &dyn GameDirectoryProbe) -> GamePrerequisiteReport {
        unreachable!("package contents query never inspects prerequisites")
    }

    fn allowed_install_roots(&self) -> Vec<String> {
        self.allowed_roots.clone()
    }

    fn is_rejected_install_file_name(&self, file_name: &str) -> bool {
        self.rejected_extensions.iter().any(|extension| {
            file_name
                .to_ascii_lowercase()
                .ends_with(&format!(".{extension}"))
        })
    }
}

struct StubRepository;

impl ModImportResultRepository for StubRepository {
    fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> Result<()> {
        unreachable!("package contents query never writes")
    }

    fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
        unreachable!("package contents query never lists analyses")
    }

    fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
        if mod_id != "mod-a" {
            return Ok(None);
        }
        Ok(Some(StoredModImportAnalysis {
            mod_id: "mod-a".to_owned(),
            task_id: "task-a".to_owned(),
            package_id: "package-a".to_owned(),
            display_name: "包 A".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
        }))
    }
}

struct StubContentRootChoices;

impl ModPackageContentRootRepository for StubContentRootChoices {
    fn load_content_root(&self, _package_id: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn save_content_root(&self, _package_id: &str, _content_root: &str) -> Result<()> {
        Ok(())
    }

    fn clear_content_root(&self, _package_id: &str) -> Result<()> {
        Ok(())
    }
}

struct StubLocator;

impl ModImportSandboxLocator for StubLocator {
    fn sandbox_root_for_package(&self, package_id: &str) -> Result<PathBuf> {
        Ok(PathBuf::from("/sandbox").join(package_id))
    }
}

struct StubScanner {
    contents: ModPackageContents,
}

impl ModPackageContentScanner for StubScanner {
    fn scan_package_contents(
        &self,
        _request: ModPackageContentScanRequest<'_>,
    ) -> Result<ModPackageContents, ModPackageInstallFileScanError> {
        Ok(self.contents.clone())
    }
}

fn entry(package_file_id: &str) -> ModPackageContentEntry {
    ModPackageContentEntry {
        package_file_id: package_file_id.to_owned(),
        size_bytes: 1,
    }
}

fn service(contents: ModPackageContents) -> PackageContentsQueryService {
    PackageContentsQueryService::with_imported_mod_sources(
        Arc::new(StubRepository),
        Arc::new(StubLocator),
        Arc::new(StubScanner { contents }),
        Arc::new(StubContentRootChoices),
        vec![Arc::new(StubAdapter {
            allowed_roots: vec!["nativePC".to_owned()],
            rejected_extensions: vec!["exe".to_owned()],
        })],
    )
}

fn query(contents: ModPackageContents) -> PackageContents {
    service(contents)
        .query(PackageContentsQueryRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new("mod-a"),
        })
        .expect("package contents query")
}

/*
 * `#354` D1 的核心断言：三条事实**各自独立**，不合并成一个 `will_install`。
 *
 * `.exe` 落在 `nativePC/` 之下，因此 `installable` 为真——那是它在**普通安装**链路上的实况
 * （拒绝清单当前只作用于重定向链路，缺口记在 `executable_reject_list.rs` 模块头）。同时
 * `rejected_by_game` 为真。两个字段同时为真正是要如实报出去的状态；合并成单一结论必然在
 * 某一条链路上给出与实际相反的答案。
 */
#[test]
fn the_reject_list_hit_and_installability_are_reported_as_separate_facts() {
    let contents = query(ModPackageContents {
        entries: vec![
            entry("nativePC/wp/two/bs_two012/mod/bs_two012.mod3"),
            entry("nativePC/wp/two/bs_two012/mod/MHWTexConverter_by_Jodo.exe"),
        ],
        content_root: ModPackageContentRoot::Fallback,
        candidates: Vec::new(),
    });

    let model = &contents.entries[0];
    assert_eq!(
        model.target_path.as_deref(),
        Some("nativePC/wp/two/bs_two012/mod/bs_two012.mod3")
    );
    assert!(model.installable);
    assert!(!model.rejected_by_game);

    let executable = &contents.entries[1];
    assert!(
        executable.installable,
        "普通安装链路当前确实会装它——事实就要如实报，不能拿拒绝清单把它涂成不可安装"
    );
    assert!(executable.rejected_by_game);
}

/// 内容根有歧义时，目标路径**算不出来**，但文件仍然逐条列出——D2 让玩家挑完就有了。
#[test]
fn an_ambiguous_content_root_yields_entries_without_target_paths() {
    let contents = query(ModPackageContents {
        entries: vec![entry("大剑/nativePC/wp/two003.mod3"), entry("readme.txt")],
        content_root: ModPackageContentRoot::Ambiguous(vec!["大剑".to_owned(), "太刀".to_owned()]),
        candidates: Vec::new(),
    });

    assert_eq!(
        contents.content_root,
        PackageContentRoot::Ambiguous(vec!["大剑".to_owned(), "太刀".to_owned()])
    );
    assert_eq!(contents.entries.len(), 2);
    for item in &contents.entries {
        assert_eq!(item.target_path, None);
        assert!(!item.installable);
    }
}

/// 包装目录之外的文件照常列出，但不在内容根之下，因此没有目标路径、不可安装。
#[test]
fn files_outside_the_content_root_are_listed_without_a_target_path() {
    let contents = query(ModPackageContents {
        entries: vec![
            entry("readme.txt"),
            entry("黑骑士大剑/nativePC/wp/two003.mod3"),
            entry("黑骑士大剑/预览.png"),
        ],
        content_root: ModPackageContentRoot::Single("黑骑士大剑".to_owned()),
        candidates: Vec::new(),
    });

    assert_eq!(contents.entries[0].target_path, None);
    assert!(!contents.entries[0].installable);

    assert_eq!(
        contents.entries[1].target_path.as_deref(),
        Some("nativePC/wp/two003.mod3")
    );
    assert!(contents.entries[1].installable);

    // 在内容根之内但不在 `nativePC` 之下：算得出目标路径，但装不了。两条事实分开报。
    assert_eq!(contents.entries[2].target_path.as_deref(), Some("预览.png"));
    assert!(!contents.entries[2].installable);
}

/*
 * 内容根按**段**比较，不是按字符串前缀。
 *
 * `皮肤` 不得吞掉 `皮肤2/`——真按前缀切会得出 `2/x.tex` 这条来路不明的目标路径，表现是
 * 「装到别的目录去」而不是报错，属于最难发现的一类。
 */
#[test]
fn a_sibling_directory_sharing_the_content_root_prefix_is_not_swallowed() {
    let contents = query(ModPackageContents {
        entries: vec![
            entry("皮肤/nativePC/wp/two003.mod3"),
            entry("皮肤2/nativePC/wp/two019.mod3"),
        ],
        content_root: ModPackageContentRoot::Single("皮肤".to_owned()),
        candidates: Vec::new(),
    });

    assert_eq!(
        contents.entries[0].target_path.as_deref(),
        Some("nativePC/wp/two003.mod3")
    );
    assert_eq!(
        contents.entries[1].target_path, None,
        "`皮肤2` 是同级目录，不属于内容根 `皮肤`"
    );
}

#[test]
fn a_missing_mod_is_reported_distinctly_from_an_unavailable_sandbox() {
    let error = service(ModPackageContents {
        entries: Vec::new(),
        content_root: ModPackageContentRoot::Fallback,
        candidates: Vec::new(),
    })
    .query(PackageContentsQueryRequest {
        game_id: GameId::mhw(),
        mod_id: ModId::new("mod-missing"),
    })
    .expect_err("未知 Mod 必须失败");

    assert_eq!(error.code(), "package_contents_mod_not_found");
}

/// 扫描侧的稳定错误码原样透出，不在查询层重新发明一套。
#[test]
fn scan_failures_keep_their_own_stable_code() {
    struct FailingScanner;

    impl ModPackageContentScanner for FailingScanner {
        fn scan_package_contents(
            &self,
            _request: ModPackageContentScanRequest<'_>,
        ) -> Result<ModPackageContents, ModPackageInstallFileScanError> {
            Err(ModPackageInstallFileScanError::UnsupportedEntry)
        }
    }

    let error = PackageContentsQueryService::with_imported_mod_sources(
        Arc::new(StubRepository),
        Arc::new(StubLocator),
        Arc::new(FailingScanner),
        Arc::new(StubContentRootChoices),
        vec![Arc::new(StubAdapter {
            allowed_roots: vec!["nativePC".to_owned()],
            rejected_extensions: Vec::new(),
        })],
    )
    .query(PackageContentsQueryRequest {
        game_id: GameId::mhw(),
        mod_id: ModId::new("mod-a"),
    })
    .expect_err("扫描失败必须透出");

    assert_eq!(error.code(), "imported_mod_file_scan_unsupported_entry");
}

/// 记录选择的仓储：记下最后一次写入，供断言检查「存进去的到底是什么」。
#[derive(Default)]
struct RecordingContentRootChoices {
    saved: std::sync::Mutex<Option<(String, String)>>,
    cleared: std::sync::Mutex<Vec<String>>,
}

impl ModPackageContentRootRepository for RecordingContentRootChoices {
    fn load_content_root(&self, _package_id: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn save_content_root(&self, package_id: &str, content_root: &str) -> Result<()> {
        *self.saved.lock().expect("lock") = Some((package_id.to_owned(), content_root.to_owned()));
        Ok(())
    }

    fn clear_content_root(&self, package_id: &str) -> Result<()> {
        self.cleared
            .lock()
            .expect("lock")
            .push(package_id.to_owned());
        Ok(())
    }
}

fn service_with_choices(
    contents: ModPackageContents,
    choices: Arc<RecordingContentRootChoices>,
) -> PackageContentsQueryService {
    PackageContentsQueryService::with_imported_mod_sources(
        Arc::new(StubRepository),
        Arc::new(StubLocator),
        Arc::new(StubScanner { contents }),
        choices,
        vec![Arc::new(StubAdapter {
            allowed_roots: vec!["nativePC".to_owned()],
            rejected_extensions: Vec::new(),
        })],
    )
}

fn collection_contents() -> ModPackageContents {
    ModPackageContents {
        entries: vec![entry("大剑/nativePC/wp/two003.mod3")],
        content_root: ModPackageContentRoot::Ambiguous(vec!["大剑".to_owned(), "太刀".to_owned()]),
        candidates: vec![String::new(), "大剑".to_owned(), "太刀".to_owned()],
    }
}

fn request() -> PackageContentsQueryRequest {
    PackageContentsQueryRequest {
        game_id: GameId::mhw(),
        mod_id: ModId::new("mod-a"),
    }
}

/// 选择按 **package_id** 存，不是 mod_id——内容根是解压包的物理属性。
#[test]
fn a_chosen_content_root_is_persisted_against_the_package() {
    let choices = Arc::new(RecordingContentRootChoices::default());
    service_with_choices(collection_contents(), Arc::clone(&choices))
        .choose_content_root(request(), "太刀")
        .expect("候选之一必须能选中");

    assert_eq!(
        *choices.saved.lock().expect("lock"),
        Some(("package-a".to_owned(), "太刀".to_owned()))
    );
}

/*
 * 非候选的值在**设置**这一步就被拦下，不会存进去。
 *
 * 存进去等扫描时才失败关闭的话，玩家会以为选好了，直到下一次安装才发现不对——诊断出现的
 * 时机离他做决定的时机太远。
 */
#[test]
fn a_content_root_outside_the_candidate_list_is_rejected_before_it_is_stored() {
    let choices = Arc::new(RecordingContentRootChoices::default());
    let error = service_with_choices(collection_contents(), Arc::clone(&choices))
        .choose_content_root(request(), "锤")
        .expect_err("非候选必须拒绝");

    assert_eq!(
        error.code(),
        "package_contents_content_root_not_a_candidate"
    );
    assert_eq!(
        *choices.saved.lock().expect("lock"),
        None,
        "拒绝的值不得落盘"
    );
}

/// 空串是合法候选（沙箱根本身），必须能选中——它与「没传值」不是一回事。
#[test]
fn the_sandbox_root_itself_is_a_selectable_candidate() {
    let choices = Arc::new(RecordingContentRootChoices::default());
    service_with_choices(collection_contents(), Arc::clone(&choices))
        .choose_content_root(request(), "")
        .expect("沙箱根本身是合法候选");

    assert_eq!(
        *choices.saved.lock().expect("lock"),
        Some(("package-a".to_owned(), String::new()))
    );
}

#[test]
fn clearing_a_choice_targets_the_same_package() {
    let choices = Arc::new(RecordingContentRootChoices::default());
    service_with_choices(collection_contents(), Arc::clone(&choices))
        .clear_content_root(request())
        .expect("清除");

    assert_eq!(*choices.cleared.lock().expect("lock"), vec!["package-a"]);
}

/// 候选清单原样透出：玩家选定之后 `content_root` 会收敛，候选不跟着消失他才改得了主意。
#[test]
fn the_candidate_list_is_projected_independently_of_the_effective_root() {
    let contents = query(ModPackageContents {
        entries: Vec::new(),
        content_root: ModPackageContentRoot::Single("大剑".to_owned()),
        candidates: vec![String::new(), "大剑".to_owned(), "太刀".to_owned()],
    });

    assert_eq!(
        contents.content_root,
        PackageContentRoot::Single("大剑".to_owned())
    );
    assert_eq!(
        contents.candidates,
        vec![String::new(), "大剑".to_owned(), "太刀".to_owned()]
    );
}
