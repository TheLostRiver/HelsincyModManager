use super::*;
use hmm_core::{RetargetFileDisposition, RetargetFileEffect, RetargetFileReason};

fn ready(preparation: ReinstallPreparation) -> PreparedReinstall {
    match preparation {
        ReinstallPreparation::Ready(prepared) => *prepared,
        ReinstallPreparation::Blocked(_) => panic!("fixture must stay ready"),
    }
}

fn effects(prepared: &PreparedReinstall) -> Vec<RetargetFileEffect> {
    prepared
        .source_files
        .iter()
        .map(|file| RetargetFileEffect {
            package_file_id: file.provider.package_file_id.clone(),
            source_id: None,
            source_path: file.provider.target_path.clone(),
            target_path: Some(file.provider.target_path.clone()),
            disposition: RetargetFileDisposition::PackageCompanion,
            reason: RetargetFileReason::PackageResource,
        })
        .collect()
}

#[test]
fn file_effect_token_is_stable_for_order_but_changes_with_displayed_provenance() {
    let fixture = Fixture::ready();
    let prepared = fixture.prepare(default_request());
    let mut file_effects = effects(&prepared);
    let enrich = |effects| {
        ready(
            ReinstallPreparation::Ready(Box::new(prepared.clone()))
                .with_file_effects(effects)
                .unwrap(),
        )
    };
    let enriched = enrich(file_effects.clone());
    assert_ne!(enriched.plan_token, prepared.plan_token);
    assert_eq!(enriched.plan_hash, enriched.plan_token);
    assert_eq!(
        enriched
            .plan_token
            .strip_prefix("reinstall-preview-v1:")
            .unwrap()
            .len(),
        64
    );
    file_effects.reverse();
    assert_eq!(enrich(file_effects.clone()).plan_token, enriched.plan_token);
    // 文件保留原因也是用户确认的事实，不能在重新摘要时遗漏。
    file_effects[0].disposition = RetargetFileDisposition::KeptInPlace;
    file_effects[0].reason = RetargetFileReason::UnmappedResource;
    assert_ne!(enrich(file_effects).plan_token, enriched.plan_token);
    assert_eq!(enrich(Vec::new()).plan_token, prepared.plan_token);
    fixture.assert_zero_mutations();
}

#[test]
fn source_fingerprints_extend_the_versioned_token_without_losing_file_effect_facts() {
    let fixture = Fixture::ready();
    let mut prepared = fixture.prepare(default_request());
    prepared.intent = hmm_core::ReinstallIntent::ReapplyEquipmentTargets;
    let file_effects = effects(&prepared);
    let enriched = ready(
        ReinstallPreparation::Ready(Box::new(prepared.clone()))
            .with_file_effects(file_effects)
            .unwrap(),
    );
    let summaries = enriched
        .source_files
        .iter()
        .map(|file| (file.provider.package_file_id.clone(), file.summary.clone()))
        .collect::<BTreeMap<_, _>>();
    let bind = |prepared: PreparedReinstall, summaries| {
        ready(
            ReinstallPreparation::Ready(Box::new(prepared))
                .with_reapply_source_fingerprints(summaries)
                .unwrap(),
        )
    };
    let bound = bind(enriched.clone(), summaries.clone());
    assert_ne!(bound.plan_token, enriched.plan_token);
    assert_eq!(bound.plan_hash, bound.plan_token);
    assert_eq!(
        bound
            .plan_token
            .strip_prefix("reinstall-preview-v1:")
            .unwrap()
            .len(),
        64
    );
    assert_eq!(
        bound.plan_token,
        bind(enriched.clone(), summaries.clone()).plan_token
    );
    assert_ne!(
        bound.plan_token,
        bind(prepared, summaries.clone()).plan_token
    );
    let mut changed = summaries;
    changed.values_mut().next().unwrap().sha256 = "f".repeat(64);
    assert_ne!(bound.plan_token, bind(enriched, changed).plan_token);
    fixture.assert_zero_mutations();
}
