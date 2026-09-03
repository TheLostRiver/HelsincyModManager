//! 外部 MOD 接管（adopt）的认领集派生（#286 最后一片的纯逻辑部分）。
//!
//! 接管 = **只写安装清单，不碰任何文件**：把游戏目录里「与导入包一致且无主」的文件
//! 记成本 MOD 的清单条目，之后卸载/重装走既有路径。本模块无 IO、无锁，只把两侧事实
//! （扫描的每文件状态 + 清单归属）变成一份可直接落盘的认领计划；拿锁、复验、写清单、
//! 审计在 `hmm-runtime` 里编排。
//!
//! ## 认领规则（维护者 2026-09-03 拍板，#286 评论 5522790071）
//!
//! 1. 被清单里**任何**其他条目占用的路径绝不认领——双清单认领同一路径会让 #272/#278
//!    的占用判定失效。这里用写门禁的 fail-closed 口径「任一条目即占用」，而不是归因展示
//!    用的「首条归属」（排障手册 4.9）：两者对「有没有主」结论一致，adopt 不关心主是谁。
//! 2. `changed` 不认领：认领意味着卸载会删掉一个本工具没写过、内容也不匹配包的文件且无法
//!    恢复；要包版本走重装（重装会把改动版存成 `previous_bytes`）。
//! 3. `unreadable` **阻断整次接管**：残缺事实上不建清单。#305 之后它还覆盖「沙箱副本读不到」，
//!    此时安装/卸载语义本就不可靠。
//! 4. 可认领集为空拒绝接管（与 #285「空计划必须失败」同一口径）；条目无 `backup_ref`、
//!    带 `installed_file` 摘要（卸载前靠它核对再删，缺摘要的条目永远卸不掉）、
//!    `adopted = true`。
//!
//! 推论：**可认领集 = matched ∧ 无主**。missing 不认领（文件不在，没东西可认）。

use hmm_core::{
    ExternalFileState, FileLayer, InstallManifest, InstallManifestEntry, InstallManifestStatus,
    InstallManifestStatusConsumption, InstallTargetPath, InstalledFileSummary, ModId,
    PackageFileId, ProfileId,
};

use crate::external_state_scan::PreparedExternalTarget;

/// 一条将要写进清单的认领。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAdoptClaim {
    pub target_path: InstallTargetPath,
    pub package_file_id: PackageFileId,
    /// 扫描时读到的游戏目录文件摘要。matched ⇒ 与包副本相等，也就是卸载时要核对的那份。
    pub installed_file: InstalledFileSummary,
}

/// 派生出的认领计划：认领什么、为什么没认领其余的。计数只用于界面与审计（都是数字，不含路径）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAdoptPlan {
    pub claims: Vec<ExternalAdoptClaim>,
    /// 与包一致但已被其他 MOD 的清单条目占用。
    pub skipped_claimed_count: usize,
    /// 内容与包不同（无论有主无主）。
    pub skipped_changed_count: usize,
    /// 游戏目录里不存在。
    pub skipped_missing_count: usize,
}

impl ExternalAdoptPlan {
    /// 认领条目：无 `backup_ref`（卸载 = 删除）、带摘要、`adopted` 标记；`revision_id` 为 `None`
    /// 与 GUI 安装（`start_install_task`）写出的条目同形，同一 MOD 不得混用 Some/None。
    pub fn manifest_entries(&self, mod_id: &ModId, layer: &FileLayer) -> Vec<InstallManifestEntry> {
        self.claims
            .iter()
            .map(|claim| InstallManifestEntry {
                target_path: claim.target_path.clone(),
                mod_id: mod_id.clone(),
                revision_id: None,
                package_file_id: claim.package_file_id.clone(),
                layer: layer.clone(),
                backup_ref: None,
                installed_file: Some(claim.installed_file.clone()),
                adopted: true,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExternalAdoptPlanError {
    /// 规则 3：任一侧读不到就不建清单。
    #[error("{count} compared file(s) are unreadable; adopt refuses to build a manifest on partial facts")]
    UnreadableFiles { count: usize },
    /// 规则 4：没有任何可认领的文件（全缺失 / 全被改动 / 全被占用，或比对集为空）。
    #[error("no file is both matched and unclaimed; nothing to adopt")]
    NothingToAdopt,
    /// 该 MOD 在清单里已有条目：它已经是「HMM 装的」，接管无从谈起，该走重装。
    #[error("the mod already owns manifest entries; use reinstall instead of adopt")]
    AlreadyInstalled,
    /// 清单处于进行中/失败态，entries 不可信，不能在它上面追加。
    #[error(
        "the profile manifest status {status:?} does not trust entries; resolve recovery first"
    )]
    ManifestNotTrusted { status: InstallManifestStatus },
    /// 不变量被破坏：matched 的文件必然读到过游戏侧摘要。
    #[error("a matched file has no game-side summary; scan facts are inconsistent")]
    MissingGameFileSummary,
    /// 不变量被破坏：三个输入序列必须与比对集等长同序。
    #[error("scan facts have inconsistent lengths")]
    InconsistentFacts,
}

/// 从扫描事实与清单派生认领计划。
///
/// `prepared` / `file_states` / `game_files` 与比对集**同序等长**（扫描服务与存储的不变量，
/// #305 之后不再因读失败缩短）；`game_files[i]` 是游戏侧摘要，缺失/读不到时为 `None`。
/// `manifest` 为 `None` 表示该配置档从未有 HMM 安装。
///
/// 调用方必须在写锁内、以**当下**读到的清单调用它——guard 不等于授权，清单事实要在锁内重验。
pub fn derive_external_adopt_plan(
    mod_id: &ModId,
    prepared: &[PreparedExternalTarget],
    file_states: &[ExternalFileState],
    game_files: &[Option<InstalledFileSummary>],
    manifest: Option<&InstallManifest>,
) -> Result<ExternalAdoptPlan, ExternalAdoptPlanError> {
    if prepared.len() != file_states.len() || prepared.len() != game_files.len() {
        return Err(ExternalAdoptPlanError::InconsistentFacts);
    }

    if let Some(manifest) = manifest {
        if manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries {
            return Err(ExternalAdoptPlanError::ManifestNotTrusted {
                status: manifest.status,
            });
        }
        if manifest.entries.iter().any(|entry| entry.mod_id == *mod_id) {
            return Err(ExternalAdoptPlanError::AlreadyInstalled);
        }
    }

    let unreadable_count = file_states
        .iter()
        .filter(|state| **state == ExternalFileState::Unreadable)
        .count();
    if unreadable_count > 0 {
        return Err(ExternalAdoptPlanError::UnreadableFiles {
            count: unreadable_count,
        });
    }

    let mut plan = ExternalAdoptPlan {
        claims: Vec::new(),
        skipped_claimed_count: 0,
        skipped_changed_count: 0,
        skipped_missing_count: 0,
    };
    for ((target, state), game_file) in prepared.iter().zip(file_states).zip(game_files) {
        match state {
            ExternalFileState::Missing => plan.skipped_missing_count += 1,
            ExternalFileState::Changed => plan.skipped_changed_count += 1,
            // 上面已整体拒绝；留这一臂是为了 match 穷尽，而不是默默归到某一类。
            ExternalFileState::Unreadable => {
                return Err(ExternalAdoptPlanError::UnreadableFiles { count: 1 });
            }
            ExternalFileState::Matched => {
                if target_is_claimed(manifest, &target.target_path) {
                    plan.skipped_claimed_count += 1;
                    continue;
                }
                let Some(installed_file) = game_file else {
                    return Err(ExternalAdoptPlanError::MissingGameFileSummary);
                };
                plan.claims.push(ExternalAdoptClaim {
                    target_path: target.target_path.clone(),
                    package_file_id: PackageFileId::new(target.package_file_id.clone()),
                    installed_file: installed_file.clone(),
                });
            }
        }
    }

    if plan.claims.is_empty() {
        return Err(ExternalAdoptPlanError::NothingToAdopt);
    }
    Ok(plan)
}

/// 写门禁口径：清单里**任何**条目引用了该路径就算占用。
///
/// 与 `install::cross_mod_target_conflicts` 同一语义（任一条目即冲突），刻意**不**复用
/// 归因用的 `first_manifest_entry_by_target`——那是「归谁」的展示视图，畸形态下只报首条
/// （排障手册 4.9）。已安装 MOD 自己的条目在此之前已被 `AlreadyInstalled` 拒绝，所以这里
/// 不需要区分「别人的」和「自己的」。
fn target_is_claimed(manifest: Option<&InstallManifest>, target_path: &InstallTargetPath) -> bool {
    manifest.is_some_and(|manifest| {
        manifest
            .entries
            .iter()
            .any(|entry| entry.target_path == *target_path)
    })
}

/// 把认领条目**追加**进配置档清单，其余字段原样保留（状态、schema、replacement_bindings、
/// `plan_hash`——它描述的是上一次安装计划，那些条目都还在，不因追加而失效；卸载置 `None`
/// 是因为它移走了条目）。只更新 `completed_at`。
///
/// 追加前再核一次路径不重叠：派生阶段已保证，这里是最后一道结构性防线——同一路径两条
/// 条目正是 #278 占用判定的畸形态。
pub fn append_adopted_entries(
    profile_id: &ProfileId,
    existing: Option<InstallManifest>,
    entries: Vec<InstallManifestEntry>,
    completed_at: String,
) -> Result<InstallManifest, ExternalAdoptPlanError> {
    let mut manifest = existing.unwrap_or_else(|| {
        InstallManifest::completed_with_metadata(
            profile_id.clone(),
            Vec::new(),
            None,
            Some(completed_at.clone()),
            None,
            None,
        )
    });
    if entries.iter().any(|entry| {
        manifest
            .entries
            .iter()
            .any(|existing| existing.target_path == entry.target_path)
    }) {
        return Err(ExternalAdoptPlanError::InconsistentFacts);
    }
    manifest.entries.extend(entries);
    manifest.completed_at = Some(completed_at);
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{ExternalFileState as State, InstallManifestEntry};

    fn roots() -> Vec<String> {
        vec!["nativePC".to_owned()]
    }

    fn target(relative: &str) -> PreparedExternalTarget {
        PreparedExternalTarget {
            target_path: InstallTargetPath::parse(relative, roots()).expect("valid target"),
            package_file_id: relative.to_owned(),
            display_path: relative.to_owned(),
        }
    }

    fn summary(tag: &str) -> InstalledFileSummary {
        InstalledFileSummary {
            size_bytes: 3,
            sha256: format!("hash-{tag}"),
        }
    }

    fn entry_for(relative: &str, mod_id: &str) -> InstallManifestEntry {
        InstallManifestEntry {
            target_path: InstallTargetPath::parse(relative, roots()).expect("valid target"),
            mod_id: ModId::new(mod_id),
            revision_id: None,
            package_file_id: PackageFileId::new(relative),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: Some(summary("other")),
            adopted: false,
        }
    }

    fn manifest_with(entries: Vec<InstallManifestEntry>) -> InstallManifest {
        InstallManifest::completed_with_metadata(
            ProfileId::new("default"),
            entries,
            None,
            None,
            None,
            None,
        )
    }

    fn scanned() -> ModId {
        ModId::new("mod-scanned")
    }

    #[test]
    fn matched_and_unclaimed_files_are_the_whole_claim_set() {
        let prepared = [target("nativePC/a.mod3"), target("nativePC/b.mrl3")];
        let states = [State::Matched, State::Matched];
        let files = [Some(summary("a")), Some(summary("b"))];

        let plan = derive_external_adopt_plan(&scanned(), &prepared, &states, &files, None)
            .expect("two matched unclaimed files adopt");

        assert_eq!(plan.claims.len(), 2);
        assert_eq!(plan.claims[0].target_path.as_str(), "nativePC/a.mod3");
        assert_eq!(plan.claims[0].installed_file, summary("a"));
        assert_eq!(plan.claims[1].installed_file, summary("b"));
        assert_eq!(plan.skipped_claimed_count, 0);
        assert_eq!(plan.skipped_changed_count, 0);
        assert_eq!(plan.skipped_missing_count, 0);
    }

    #[test]
    fn rule_1_files_claimed_by_another_mod_are_never_adopted() {
        // 字节相同的双胞胎：flat 已由 HMM 安装，扫 wrapped 两个文件都 matched 且都有主。
        let prepared = [target("nativePC/a.mod3"), target("nativePC/b.mrl3")];
        let states = [State::Matched, State::Matched];
        let files = [Some(summary("a")), Some(summary("b"))];
        let manifest = manifest_with(vec![entry_for("nativePC/a.mod3", "mod-flat")]);

        let plan =
            derive_external_adopt_plan(&scanned(), &prepared, &states, &files, Some(&manifest))
                .expect("one file is still unclaimed");

        assert_eq!(plan.claims.len(), 1);
        assert_eq!(plan.claims[0].target_path.as_str(), "nativePC/b.mrl3");
        assert_eq!(plan.skipped_claimed_count, 1);
    }

    #[test]
    fn rule_1_uses_any_entry_not_the_first_entry_view() {
        // 畸形态：同一路径两条异主条目。首条归属视图会说「归 mod-x」，写门禁只需要知道
        // 「有主」；无论哪条在前都不得认领。
        let prepared = [target("nativePC/a.mod3")];
        let states = [State::Matched];
        let files = [Some(summary("a"))];
        let manifest = manifest_with(vec![
            entry_for("nativePC/a.mod3", "mod-x"),
            entry_for("nativePC/a.mod3", "mod-y"),
        ]);

        let error =
            derive_external_adopt_plan(&scanned(), &prepared, &states, &files, Some(&manifest))
                .expect_err("claimed path must not be adopted");

        assert_eq!(error, ExternalAdoptPlanError::NothingToAdopt);
    }

    #[test]
    fn rule_2_changed_files_are_skipped_even_when_unclaimed() {
        let prepared = [target("nativePC/a.mod3"), target("nativePC/b.mrl3")];
        let states = [State::Matched, State::Changed];
        let files = [Some(summary("a")), Some(summary("b-changed"))];

        let plan = derive_external_adopt_plan(&scanned(), &prepared, &states, &files, None)
            .expect("the matched file still adopts");

        assert_eq!(plan.claims.len(), 1);
        assert_eq!(plan.claims[0].target_path.as_str(), "nativePC/a.mod3");
        assert_eq!(plan.skipped_changed_count, 1);
    }

    #[test]
    fn rule_3_any_unreadable_file_blocks_the_whole_adopt() {
        let prepared = [target("nativePC/a.mod3"), target("nativePC/b.mrl3")];
        let states = [State::Matched, State::Unreadable];
        let files = [Some(summary("a")), None];

        let error = derive_external_adopt_plan(&scanned(), &prepared, &states, &files, None)
            .expect_err("unreadable must block");

        assert_eq!(error, ExternalAdoptPlanError::UnreadableFiles { count: 1 });
    }

    #[test]
    fn rule_4_all_missing_is_nothing_to_adopt_and_missing_is_counted() {
        let prepared = [target("nativePC/a.mod3"), target("nativePC/b.mrl3")];
        let states = [State::Missing, State::Missing];
        let files = [None, None];

        let error = derive_external_adopt_plan(&scanned(), &prepared, &states, &files, None)
            .expect_err("nothing on disk to adopt");
        assert_eq!(error, ExternalAdoptPlanError::NothingToAdopt);

        // 部分缺失时计数进 skipped_missing，认领集只含 matched。
        let states = [State::Matched, State::Missing];
        let files = [Some(summary("a")), None];
        let plan = derive_external_adopt_plan(&scanned(), &prepared, &states, &files, None)
            .expect("one matched file adopts");
        assert_eq!(plan.claims.len(), 1);
        assert_eq!(plan.skipped_missing_count, 1);
    }

    #[test]
    fn an_empty_comparison_set_is_nothing_to_adopt() {
        let error = derive_external_adopt_plan(&scanned(), &[], &[], &[], None)
            .expect_err("empty set must not adopt");
        assert_eq!(error, ExternalAdoptPlanError::NothingToAdopt);
    }

    #[test]
    fn a_mod_that_already_owns_entries_is_refused_before_anything_else() {
        let prepared = [target("nativePC/a.mod3")];
        let states = [State::Matched];
        let files = [Some(summary("a"))];
        let manifest = manifest_with(vec![entry_for("nativePC/z.mod3", "mod-scanned")]);

        let error =
            derive_external_adopt_plan(&scanned(), &prepared, &states, &files, Some(&manifest))
                .expect_err("installed mods reinstall, not adopt");

        assert_eq!(error, ExternalAdoptPlanError::AlreadyInstalled);
    }

    #[test]
    fn a_manifest_that_does_not_trust_entries_is_refused() {
        let prepared = [target("nativePC/a.mod3")];
        let states = [State::Matched];
        let files = [Some(summary("a"))];
        for status in [
            InstallManifestStatus::Planned,
            InstallManifestStatus::Committing,
            InstallManifestStatus::RollbackRequired,
            InstallManifestStatus::RepairRequired,
        ] {
            let mut manifest = manifest_with(Vec::new());
            manifest.status = status;
            let error =
                derive_external_adopt_plan(&scanned(), &prepared, &states, &files, Some(&manifest))
                    .expect_err("untrusted manifest must refuse");
            assert_eq!(error, ExternalAdoptPlanError::ManifestNotTrusted { status });
        }
        // rolled_back 的 entries 仍可信，允许追加。
        let mut manifest = manifest_with(Vec::new());
        manifest.status = InstallManifestStatus::RolledBack;
        derive_external_adopt_plan(&scanned(), &prepared, &states, &files, Some(&manifest))
            .expect("rolled_back trusts entries");
    }

    #[test]
    fn a_matched_file_without_a_game_side_summary_is_an_invariant_violation() {
        let prepared = [target("nativePC/a.mod3")];
        let states = [State::Matched];
        let files = [None];

        let error = derive_external_adopt_plan(&scanned(), &prepared, &states, &files, None)
            .expect_err("matched without summary is inconsistent");
        assert_eq!(error, ExternalAdoptPlanError::MissingGameFileSummary);

        let error = derive_external_adopt_plan(&scanned(), &prepared, &states, &[], None)
            .expect_err("length mismatch is inconsistent");
        assert_eq!(error, ExternalAdoptPlanError::InconsistentFacts);
    }

    #[test]
    fn manifest_entries_have_no_backup_ref_a_summary_and_the_adopted_flag() {
        let prepared = [target("nativePC/a.mod3")];
        let states = [State::Matched];
        let files = [Some(summary("a"))];
        let plan = derive_external_adopt_plan(&scanned(), &prepared, &states, &files, None)
            .expect("adopts");

        let entries = plan.manifest_entries(&scanned(), &FileLayer::new("base", 0));

        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.mod_id, scanned());
        assert_eq!(entry.target_path.as_str(), "nativePC/a.mod3");
        assert_eq!(entry.package_file_id, PackageFileId::new("nativePC/a.mod3"));
        assert_eq!(entry.layer, FileLayer::new("base", 0));
        // 卸载 = 删除（无 backup_ref）；卸载前核对靠摘要；来源标记为接管；GUI 安装同款 None revision。
        assert_eq!(entry.backup_ref, None);
        assert_eq!(entry.installed_file, Some(summary("a")));
        assert!(entry.adopted);
        assert_eq!(entry.revision_id, None);
    }

    #[test]
    fn append_keeps_the_existing_manifest_intact_and_refuses_overlapping_paths() {
        let existing = {
            let mut manifest = manifest_with(vec![entry_for("nativePC/z.mod3", "mod-other")]);
            manifest.plan_hash = Some("plan-of-other".to_owned());
            manifest.created_at = Some("unix:1".to_owned());
            manifest
        };
        let plan = derive_external_adopt_plan(
            &scanned(),
            &[target("nativePC/a.mod3")],
            &[State::Matched],
            &[Some(summary("a"))],
            Some(&existing),
        )
        .expect("adopts");
        let entries = plan.manifest_entries(&scanned(), &FileLayer::new("base", 0));

        let merged = append_adopted_entries(
            &ProfileId::new("default"),
            Some(existing.clone()),
            entries.clone(),
            "unix:2".to_owned(),
        )
        .expect("append");

        assert_eq!(merged.entries.len(), 2);
        assert_eq!(merged.entries[0], existing.entries[0], "既有条目原样保留");
        assert!(merged.entries[1].adopted);
        assert_eq!(merged.plan_hash.as_deref(), Some("plan-of-other"));
        assert_eq!(merged.created_at.as_deref(), Some("unix:1"));
        assert_eq!(merged.completed_at.as_deref(), Some("unix:2"));
        assert_eq!(merged.status, InstallManifestStatus::Completed);
        merged.validate().expect("merged manifest validates");

        // 清单不存在（该配置档从未安装过）时新建一份 completed 清单。
        let fresh = append_adopted_entries(
            &ProfileId::new("default"),
            None,
            entries.clone(),
            "unix:3".to_owned(),
        )
        .expect("fresh manifest");
        assert_eq!(fresh.profile_id, ProfileId::new("default"));
        assert_eq!(fresh.entries, entries);

        // 结构性防线：同一路径已有条目时拒绝追加。
        let error = append_adopted_entries(
            &ProfileId::new("default"),
            Some(merged),
            entries,
            "unix:4".to_owned(),
        )
        .expect_err("overlap must be refused");
        assert_eq!(error, ExternalAdoptPlanError::InconsistentFacts);
    }
}
