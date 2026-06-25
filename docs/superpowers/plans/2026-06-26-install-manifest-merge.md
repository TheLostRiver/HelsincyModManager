# Install Manifest Merge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 InstallPlan MVP 补上 manifest 读取与按目标路径合并能力，避免一次安装覆盖同一 profile 下其他已记录的安装条目。

**Architecture:** `hmm-ports` 扩展 `InstallManifestRepository` 只读接口；`hmm-infra` 的 JSON repository 负责安全读取 `profileId.json`；`hmm-app` 的 `InstallCommitService` 在真实写入前读取旧 manifest，并在保存前按目标路径替换旧条目、保留未触达条目。替换已有托管目标时，manifest 继承旧条目的长期 `backup_ref`，本次提交产生的中间状态 backup 只作为 pending rollback 资源并在成功后 best-effort 清理。此 PR 不新增 Tauri command、不接前端 UI、不实现卸载或 repair 状态。

**Tech Stack:** Rust workspace, `anyhow`, `serde_json`, existing temp/fake test fixtures, PowerShell verification scripts.

---

### Task 1: Port And App Tests

**Files:**
- Modify: `src-tauri/crates/hmm-ports/src/install.rs`
- Modify: `src-tauri/crates/hmm-app/src/install.rs`

- [x] **Step 1: Write the failing app test**

Add a test in `src-tauri/crates/hmm-app/src/install.rs` proving commit reads an existing manifest, preserves entries whose target path is not touched, and replaces entries whose target path is written by the current plan.

- [x] **Step 2: Verify the app test fails**

Run:

```powershell
cargo test -p hmm-app commit_plan_merges_existing_manifest_by_target_path
```

Expected: compile failure or test failure because `InstallManifestRepository` has no `load_manifest` method and `InstallCommitService` does not merge existing manifest entries.

- [x] **Step 3: Add the minimal port and app implementation**

Add `load_manifest(&self, profile_id: &ProfileId) -> Result<Option<InstallManifest>>` to the port. In `commit_plan`, call it before reading source or writing game files. Merge by removing old entries whose `target_path` appears in the current applied changes, then append current applied entries.

- [x] **Step 4: Verify the app test passes**

Run:

```powershell
cargo test -p hmm-app commit_plan_merges_existing_manifest_by_target_path
```

Expected: PASS.

### Task 2: Manifest Read Failure Safety

**Files:**
- Modify: `src-tauri/crates/hmm-app/src/install.rs`

- [x] **Step 1: Write the failing safety test**

Add a test proving a manifest read failure aborts before reading source files or writing game files.

- [x] **Step 2: Verify the safety test fails**

Run:

```powershell
cargo test -p hmm-app commit_plan_aborts_before_writes_when_manifest_load_fails
```

Expected: FAIL because `commit_plan` currently has no read phase.

- [x] **Step 3: Implement the minimal error mapping**

Return `InstallCommitError::Failed { phase: InstallCommitPhase::ManifestRead }` when loading the existing manifest fails before any file operation.

- [x] **Step 4: Verify the safety test passes**

Run:

```powershell
cargo test -p hmm-app commit_plan_aborts_before_writes_when_manifest_load_fails
```

Expected: PASS.

### Task 3: Infra JSON Repository Read

**Files:**
- Modify: `src-tauri/crates/hmm-infra/src/install_commit.rs`

- [x] **Step 1: Write failing infra tests**

Add tests for `JsonInstallManifestRepository::load_manifest`: missing profile returns `None`; saved manifest round-trips; unsafe profile id is rejected without exposing paths; broken symlink manifests and profile id mismatches are rejected.

- [x] **Step 2: Verify infra tests fail**

Run:

```powershell
cargo test -p hmm-infra json_manifest_repository_load
```

Expected: compile failure because the method is not implemented.

- [x] **Step 3: Implement safe JSON manifest loading**

Use the existing `manifest_file_name`, containment helpers, `fs::read_to_string`, and `serde_json::from_str`. Return `Ok(None)` when the manifest file does not exist.

- [x] **Step 4: Verify infra tests pass**

Run:

```powershell
cargo test -p hmm-infra json_manifest_repository_load
```

Expected: PASS.

### Task 4: Documentation And Verification

**Files:**
- Modify: `docs/INSTALL_PLAN_STATUS.md`
- Modify: `docs/INSTALL_PLAN_MVP_TODO.md`

- [x] **Step 1: Update Chinese InstallPlan docs**

Document that manifest storage can now read the existing profile manifest and merge by target path during commit. Keep manifest query, frontend installed-state display, uninstall, repair, and rich status machine listed as future work.

- [x] **Step 2: Run focused checks**

Run:

```powershell
cargo test -p hmm-app
cargo test -p hmm-infra
cargo check --workspace
git diff --check
```

Expected: all pass.

- [x] **Step 3: Run full project verification if focused checks are clean**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: exit code 0.
