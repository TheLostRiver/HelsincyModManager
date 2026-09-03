use hmm_ports::{GameDirectoryProbe, GameDirectoryProbeFactory};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 单次试写可能被杀毒/索引器的瞬时句柄干扰（尤其 remove 一步的 sharing violation），
/// 把瞬态失败当成"不可写"会误拒真实可写的目录。有界重试下两类目录仍然快速失败：
/// 真正只读的目录每次 attempt 都立刻失败；只有瞬态干扰会在后续 attempt 恢复。
const WRITABLE_PROBE_ATTEMPTS: usize = 3;
const WRITABLE_PROBE_RETRY_DELAY: Duration = Duration::from_millis(50);

pub struct RealGameDirectoryProbe {
    root_dir: PathBuf,
}

impl RealGameDirectoryProbe {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    fn join_relative(&self, relative_path: &str) -> PathBuf {
        self.root_dir.join(relative_path)
    }
}

impl GameDirectoryProbe for RealGameDirectoryProbe {
    fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    fn root_exists(&self) -> bool {
        self.root_dir.is_dir()
    }

    fn exists(&self, relative_path: &str) -> bool {
        self.join_relative(relative_path).exists()
    }

    fn is_file(&self, relative_path: &str) -> bool {
        self.join_relative(relative_path).is_file()
    }

    fn is_dir(&self, relative_path: &str) -> bool {
        self.join_relative(relative_path).is_dir()
    }

    fn read_text_file(&self, relative_path: &str) -> anyhow::Result<String> {
        Ok(std::fs::read_to_string(self.join_relative(relative_path))?)
    }

    fn sha256_hex(&self, relative_path: &str) -> anyhow::Result<String> {
        let bytes = std::fs::read(self.join_relative(relative_path))?;
        let digest = Sha256::digest(bytes);
        Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    /// Windows 上只看只读属性判断不出可写性——真正决定权限的是 ACL、
    /// 目录所有权和安全软件的锁。唯一可靠的办法是实际试写一次。
    ///
    /// 这个探针刻意做到不可能破坏玩家数据：
    /// - 文件名带纳秒时间戳，且用 `create_new`，已存在就失败而不是覆盖；
    /// - 只写在游戏根目录下，不进 `nativePC`；
    /// - **只删除本次确实创建出来的文件**。创建失败时该路径可能是遗留文件或
    ///   并发探针的产物，删它等于删一个不属于本次探测的文件。
    ///
    /// 删除失败同样判定为不可写：安装链覆盖前要备份、卸载要移除，都需要删除权限。
    /// 能建不能删的目录若放行，只会把失败推迟到已经动过玩家文件之后。
    /// 单次删除失败可能只是安全软件的瞬时句柄，因此整体做有界重试，
    /// 每次 attempt 用全新的探针文件。
    fn root_writable(&self) -> bool {
        if !self.root_exists() {
            return false;
        }

        writable_with_retries(|| probe_root_writable_once(&self.root_dir))
    }
}

/// Same probe for any directory HMM must be able to create and delete files in (Mod storage
/// root included); the game-directory doc comment above explains the create-then-remove rules.
pub(crate) fn directory_is_writable(directory: &Path) -> bool {
    if !directory.is_dir() {
        return false;
    }
    writable_with_retries(|| probe_root_writable_once(directory))
}

/// 单次探针的三种结局。`RemoveBlocked` 携带路径：这个文件确属本次探针创建，
/// 只是删除被瞬时句柄挡住——后续 attempt 成功后要再回收它，否则瞬态干扰
/// 会在游戏根目录留下 0 字节残留，破坏 sandbox 契约测试的精确 tree baseline。
enum WritableProbeOutcome {
    Writable,
    CreateDenied,
    RemoveBlocked(PathBuf),
}

fn writable_with_retries(mut probe_once: impl FnMut() -> WritableProbeOutcome) -> bool {
    let mut blocked_paths: Vec<PathBuf> = Vec::new();
    for attempt in 0..WRITABLE_PROBE_ATTEMPTS {
        match probe_once() {
            WritableProbeOutcome::Writable => {
                for path in &blocked_paths {
                    let _ = remove_probe_file(path);
                }
                return true;
            }
            WritableProbeOutcome::CreateDenied => {}
            WritableProbeOutcome::RemoveBlocked(path) => blocked_paths.push(path),
        }
        if attempt + 1 < WRITABLE_PROBE_ATTEMPTS {
            std::thread::sleep(WRITABLE_PROBE_RETRY_DELAY);
        }
    }
    false
}

fn probe_root_writable_once(root_dir: &Path) -> WritableProbeOutcome {
    let probe_path = root_dir.join(format!(
        ".hmm-write-probe-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ));
    if !create_probe_file(&probe_path) {
        return WritableProbeOutcome::CreateDenied;
    }
    if remove_probe_file(&probe_path) {
        WritableProbeOutcome::Writable
    } else {
        WritableProbeOutcome::RemoveBlocked(probe_path)
    }
}

fn create_probe_file(probe_path: &Path) -> bool {
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(probe_path)
    {
        Ok(file) => {
            drop(file);
            true
        }
        // 创建失败时绝不碰这个路径：同名遗留文件或并发探针都会走到这里，
        // 无条件删除等于删掉一个不属于本次探测的文件。
        Err(_) => false,
    }
}

fn remove_probe_file(probe_path: &Path) -> bool {
    std::fs::remove_file(probe_path).is_ok()
}

pub struct RealGameDirectoryProbeFactory;

impl GameDirectoryProbeFactory for RealGameDirectoryProbeFactory {
    fn create(&self, directory: PathBuf) -> Box<dyn GameDirectoryProbe> {
        Box::new(RealGameDirectoryProbe::new(directory))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn probe_checks_files_relative_to_root() {
        let root = std::env::temp_dir().join(format!(
            "hmm-probe-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("nativePC")).expect("create dir");
        fs::write(root.join("MonsterHunterWorld.exe"), b"fake exe").expect("write file");

        let probe = RealGameDirectoryProbe::new(root);

        assert!(probe.root_exists());
        assert!(probe.is_file("MonsterHunterWorld.exe"));
        assert!(probe.is_dir("nativePC"));
    }

    #[test]
    fn writable_probe_reports_true_and_leaves_no_residue() {
        let root = std::env::temp_dir().join(format!(
            "hmm-probe-writable-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create dir");
        fs::write(root.join("keep.txt"), "keep").expect("write sentinel");

        let probe = RealGameDirectoryProbe::new(root.clone());
        assert!(probe.root_writable());

        // 探针必须清理干净：只留下原本就有的文件。
        let remaining = fs::read_dir(&root)
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(remaining, vec!["keep.txt".to_owned()]);
    }

    #[test]
    fn writable_probe_reports_false_for_a_missing_root() {
        // 目录不存在时必须 fail closed，不能因为"试写失败原因不明"而放行。
        let missing = std::env::temp_dir().join(format!(
            "hmm-probe-missing-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));

        assert!(!RealGameDirectoryProbe::new(missing).root_writable());
    }

    /// 瞬态 remove 失败（安全软件句柄）不等于目录不可写：单次 probe 失败、
    /// 整体 root_writable 仍应为 true。这是前置闸门在真机上不误拒的依据。
    #[cfg(windows)]
    #[test]
    fn transient_remove_failure_does_not_fail_the_writable_probe() {
        use std::os::windows::fs::OpenOptionsExt;

        let root = std::env::temp_dir().join(format!(
            "hmm-probe-transient-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create dir");

        let held_path = root.join(".hmm-write-probe-held");
        assert!(create_probe_file(&held_path));
        // share_mode(0) 打开的句柄会拒绝一切并发访问，remove_file 必须
        // 以 sharing violation 失败——这是"能建不能删"的瞬态形态。
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&held_path)
            .expect("hold probe file");
        assert!(
            !remove_probe_file(&held_path),
            "被 share_mode(0) 句柄持有的探针文件必须删不掉"
        );
        drop(held);
        assert!(remove_probe_file(&held_path), "句柄释放后同一路径必须可删");

        assert!(RealGameDirectoryProbe::new(root.clone()).root_writable());
        fs::remove_dir_all(&root).expect("cleanup");
    }

    /// 瞬态 remove 失败留下的探针文件必须被后续成功的 attempt 回收，
    /// 否则游戏根目录会多出 0 字节残留，破坏精确 tree baseline 对比。
    #[test]
    fn blocked_probe_files_are_reclaimed_after_a_successful_retry() {
        let root = std::env::temp_dir().join(format!(
            "hmm-probe-reclaim-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create dir");

        let blocked = root.join(".hmm-write-probe-blocked");
        fs::write(&blocked, b"").expect("write blocked probe residue");

        let mut outcomes = [
            WritableProbeOutcome::RemoveBlocked(blocked.clone()),
            WritableProbeOutcome::Writable,
        ]
        .into_iter();
        assert!(writable_with_retries(|| outcomes
            .next()
            .expect("planned outcomes")));

        assert!(
            !blocked.exists(),
            "被句柄挡住的探针文件在重试成功后必须被回收"
        );
        fs::remove_dir_all(&root).expect("cleanup");
    }

    /// 真正不可写的目录：create 每次都被拒，必须耗尽全部 attempt 后
    /// fail closed——重试的存在不能让只读目录被放行。
    #[test]
    fn create_denied_exhausts_attempts_and_fails_closed() {
        let attempts = std::cell::Cell::new(0u32);
        let always_denied = || {
            attempts.set(attempts.get() + 1);
            WritableProbeOutcome::CreateDenied
        };
        assert!(!writable_with_retries(always_denied));
        assert_eq!(attempts.get(), WRITABLE_PROBE_ATTEMPTS as u32);
    }

    /// 所有 attempt 都被挡住时必须 fail closed，且不再动被挡住的路径
    /// （此刻它们仍可能被外部句柄持有）。
    #[test]
    fn permanently_blocked_probes_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "hmm-probe-blocked-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create dir");
        let blocked = root.join(".hmm-write-probe-stuck");
        fs::write(&blocked, b"").expect("write stuck probe residue");

        let attempts = std::cell::Cell::new(0u32);
        let always_blocked = || {
            attempts.set(attempts.get() + 1);
            WritableProbeOutcome::RemoveBlocked(blocked.clone())
        };
        assert!(!writable_with_retries(always_blocked));
        assert_eq!(attempts.get(), WRITABLE_PROBE_ATTEMPTS as u32);
        assert!(blocked.exists(), "fail closed 时不得强行删除被挡路径");

        fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn probe_reads_text_and_computes_sha256_for_relative_files() {
        let root = std::env::temp_dir().join(format!(
            "hmm-probe-hash-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create dir");
        fs::write(root.join("notes.txt"), "abc").expect("write file");

        let probe = RealGameDirectoryProbe::new(root);

        assert_eq!(probe.read_text_file("notes.txt").expect("read text"), "abc");
        assert_eq!(
            probe.sha256_hex("notes.txt").expect("hash file"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
