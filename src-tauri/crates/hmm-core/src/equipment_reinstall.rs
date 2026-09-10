use crate::{InstallManifest, ModId, ModRevisionId, ReplacementBindingSnapshot};
use std::collections::{BTreeMap, BTreeSet};

/// 整组同版本切换：已有源不能消失或改变归属，至少一个实际目标发生变化。
/// 单绑定的历史入口继续使用自己的更窄校验。
pub fn is_same_revision_equipment_target_switch(
    manifest: &InstallManifest,
    mod_id: &ModId,
    revision_id: &ModRevisionId,
    candidates: &[ReplacementBindingSnapshot],
) -> bool {
    if candidates.len() < 2 {
        return false;
    }
    let mut installed = BTreeMap::new();
    for snapshot in manifest
        .replacement_bindings
        .iter()
        .filter(|binding| binding.mod_id() == mod_id)
    {
        if installed
            .insert(snapshot.binding().source_id(), snapshot)
            .is_some()
            || snapshot.profile_id() != &manifest.profile_id
            || snapshot
                .revision_id()
                .is_some_and(|revision| revision != revision_id)
        {
            return false;
        }
    }
    if installed.is_empty() {
        return false;
    }
    let mut sources = BTreeSet::new();
    let mut bindings = BTreeSet::new();
    let mut targets = BTreeSet::new();
    let mut changed = false;
    for candidate in candidates {
        if candidate.mod_id() != mod_id
            || candidate.profile_id() != &manifest.profile_id
            || candidate.revision_id() != Some(revision_id)
            || candidate.source_path_family() != candidate.target_path_family()
            || !sources.insert(candidate.binding().source_id())
            || !bindings.insert(candidate.binding_id())
            || !targets.insert((
                candidate.retarget_kind(),
                candidate.target_path_family(),
                candidate.target_internal_id(),
            ))
        {
            return false;
        }
        match installed.get(candidate.binding().source_id()) {
            Some(previous) => {
                if candidate.binding_id() != previous.binding_id()
                    || candidate.binding().created_at_unix_millis()
                        != previous.binding().created_at_unix_millis()
                    || candidate.source_internal_id() != previous.source_internal_id()
                    || candidate.source_path_family() != previous.source_path_family()
                    || candidate.target_path_family() != previous.target_path_family()
                    || candidate.retarget_kind() != previous.retarget_kind()
                {
                    return false;
                }
                if candidate.target_internal_id() != previous.target_internal_id() {
                    if candidate.binding().target_id() == previous.binding().target_id() {
                        return false;
                    }
                    changed = true;
                }
            }
            None => {
                // 新分析发现的源必须由应用层从同 revision 重建，不能挪用已有绑定。
                if candidate.binding().created_at_unix_millis() != 0
                    || installed
                        .values()
                        .any(|previous| previous.binding_id() == candidate.binding_id())
                {
                    return false;
                }
                changed = true;
            }
        }
    }
    changed && installed.keys().all(|source| sources.contains(source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InstallManifestStatus, ProfileId, ReplacementBinding, ReplacementBindingId,
        ReplacementSourceId, ReplacementTargetId, ReplacementTargetKind,
    };

    fn snapshot(source: &str, target: &str) -> ReplacementBindingSnapshot {
        ReplacementBindingSnapshot::new(
            ReplacementBinding::new(
                ReplacementBindingId::parse(format!("binding-{source}")).unwrap(),
                ModId::new("mod"),
                ProfileId::new("profile"),
                ReplacementSourceId::parse(format!("source-{source}")).unwrap(),
                ReplacementTargetId::parse(format!("target-{target}")).unwrap(),
                1,
            )
            .unwrap(),
            Some(ModRevisionId::new("revision")),
            source,
            target,
            "fixture/equipment",
            "fixture/equipment",
            ReplacementTargetKind::parse("weapon").unwrap(),
        )
        .unwrap()
    }

    fn manifest() -> InstallManifest {
        InstallManifest {
            profile_id: ProfileId::new("profile"),
            manifest_id: "manifest".into(),
            schema_version: 2,
            schema_migration: None,
            backend: None,
            status: InstallManifestStatus::Completed,
            created_at: None,
            completed_at: None,
            plan_hash: None,
            entries: Vec::new(),
            replacement_bindings: vec![snapshot("a", "a"), snapshot("b", "b")],
        }
    }

    fn accepts(manifest: &InstallManifest, candidates: &[ReplacementBindingSnapshot]) -> bool {
        is_same_revision_equipment_target_switch(
            manifest,
            &ModId::new("mod"),
            &ModRevisionId::new("revision"),
            candidates,
        )
    }

    #[test]
    fn changing_one_target_preserves_the_other_source_without_relaxing_the_single_source_gate() {
        let manifest = manifest();
        let candidates = vec![snapshot("a", "c"), snapshot("b", "b")];
        assert!(accepts(&manifest, &candidates));
        assert!(!crate::is_same_revision_replacement_target_switch(
            &manifest,
            &ModId::new("mod"),
            &ModRevisionId::new("revision"),
            &candidates
        ));
        assert!(!accepts(&manifest, &manifest.replacement_bindings));
        assert!(!accepts(&manifest, &candidates[..1]));
        assert!(!accepts(
            &manifest,
            &[snapshot("a", "c"), snapshot("a", "d")]
        ));
        assert!(!accepts(
            &manifest,
            &[snapshot("a", "c"), snapshot("b", "c")]
        ));
        assert!(!accepts(
            &manifest,
            &[snapshot("a", "c"), snapshot("other", "d")]
        ));
    }

    #[test]
    fn every_existing_binding_must_preserve_its_provenance() {
        let manifest = manifest();
        for (path, value) in [
            (vec!["binding", "id"], "binding-unrelated"),
            (vec!["binding", "mod_id"], "another-mod"),
            (vec!["binding", "profile_id"], "another-profile"),
            (vec!["binding", "source_id"], "source-unrelated"),
            (vec!["revision_id"], "another-revision"),
            (vec!["source_internal_id"], "another-source"),
            (vec!["source_path_family"], "fixture/another"),
            (vec!["target_path_family"], "fixture/another"),
            (vec!["retarget_kind"], "armor"),
        ] {
            let mut encoded = serde_json::to_value(snapshot("a", "c")).unwrap();
            let mut field = &mut encoded;
            for key in path {
                field = &mut field[key];
            }
            *field = serde_json::json!(value);
            let changed = serde_json::from_value(encoded).unwrap();
            assert!(
                !accepts(&manifest, &[changed, snapshot("b", "b")]),
                "provenance change accepted: {value}"
            );
        }
        let mut encoded = serde_json::to_value(snapshot("a", "c")).unwrap();
        encoded["binding"]["created_at_unix_millis"] = serde_json::json!(2);
        assert!(!accepts(
            &manifest,
            &[serde_json::from_value(encoded).unwrap(), snapshot("b", "b")]
        ));
    }

    #[test]
    fn newly_discovered_sources_require_new_identity_and_cannot_replace_an_existing_source() {
        let manifest = manifest();
        let mut candidates = vec![
            snapshot("a", "a"),
            snapshot("b", "b"),
            snapshot("extra", "extra"),
        ];
        assert!(!accepts(&manifest, &candidates));
        let mut encoded = serde_json::to_value(candidates.pop().unwrap()).unwrap();
        encoded["binding"]["created_at_unix_millis"] = serde_json::json!(0);
        candidates.push(serde_json::from_value(encoded).unwrap());
        assert!(accepts(&manifest, &candidates));
        candidates.remove(0);
        assert!(!accepts(&manifest, &candidates));
    }
}
