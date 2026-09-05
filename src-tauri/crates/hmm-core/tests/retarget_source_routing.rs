//! `#349` 切片③b：`RetargetSourceRouting` 是「某个 `package_file_id` 的字节该从哪儿读」。
//!
//! 它故意不进 `InstallPlan`（`hmm-install-plan-v1` 段对 action 逐条哈希，加字段会静默改掉
//! 所有既有 `plan_hash`），所以这份归属是**组装侧的产物**，正确性只能靠这里钉住。

use hmm_core::{
    GameId, InstallTargetPath, ModId, PackageFileId, ProfileId, ReplacementBinding,
    ReplacementBindingId, ReplacementSource, ReplacementSourceId, ReplacementTargetId,
    ReplacementTargetKind, RetargetAction, RetargetPlan, RetargetSourceRouting,
};

fn target(path: &str) -> InstallTargetPath {
    InstallTargetPath::parse(path, ["nativePC"]).expect("target")
}

fn source_id() -> ReplacementSourceId {
    ReplacementSourceId::parse("mhw:weapon:one001").expect("source id")
}

fn source() -> ReplacementSource {
    ReplacementSource::new(
        source_id(),
        GameId::mhw(),
        ReplacementTargetKind::parse("weapon").expect("kind"),
        "one001",
        "wp/one",
        true,
    )
    .expect("source")
}

fn binding(binding_id: &str, target_main: &str) -> ReplacementBinding {
    ReplacementBinding::new(
        ReplacementBindingId::parse(binding_id).expect("binding id"),
        ModId::new("mod-weapon"),
        ProfileId::new("default"),
        source_id(),
        ReplacementTargetId::parse(format!("mhw:weapon:{target_main}")).expect("target id"),
        42,
    )
    .expect("binding")
}

fn action(package_file_id: &str, target_main: &str, extension: &str) -> RetargetAction {
    RetargetAction::new(
        PackageFileId::new(package_file_id),
        target(&format!("nativePC/wp/one/one001/mod/one001.{extension}")),
        target(&format!(
            "nativePC/wp/one/{target_main}/mod/{target_main}.{extension}"
        )),
        source_id(),
        "one001",
        target_main,
        "wp/one",
        "wp/one",
    )
    .expect("action")
}

/// 一个计划的路由覆盖它**全部**动作，且每条都指向这个计划的绑定。
///
/// 漏掉任何一条，提交时那个文件就会去读沙箱原包的字节、装进重定向后的目标路径——
/// 装上去了、不报错，但内容是错的。
#[test]
fn retarget_plan_routes_every_action_to_its_own_binding() {
    let plan = RetargetPlan::new(
        binding("binding-first", "one002"),
        source(),
        vec![
            action("pair.mod3", "one002", "mod3"),
            action("pair.mrl3", "one002", "mrl3"),
        ],
        Vec::new(),
    )
    .expect("plan");

    let routing = plan.source_routing();

    assert_eq!(routing.len(), plan.actions().len());
    for action in plan.actions() {
        assert_eq!(
            routing.binding_for(action.package_file_id()),
            Some(plan.binding().id()),
        );
    }
}

/// 两个槽位各自绑定、各自 staging 根——合并后每个文件仍归属自己的绑定。
#[test]
fn merged_routing_keeps_each_binding_for_its_own_files() {
    let first = RetargetPlan::new(
        binding("binding-first", "one002"),
        source(),
        vec![action("first.mod3", "one002", "mod3")],
        Vec::new(),
    )
    .expect("first plan");
    let second = RetargetPlan::new(
        binding("binding-second", "one003"),
        source(),
        vec![action("second.mod3", "one003", "mod3")],
        Vec::new(),
    )
    .expect("second plan");

    let mut routing = first.source_routing();
    routing.merge(second.source_routing()).expect("merge");

    assert_eq!(routing.len(), 2);
    assert_eq!(
        routing.binding_for(&PackageFileId::new("first.mod3")),
        Some(first.binding().id()),
    );
    assert_eq!(
        routing.binding_for(&PackageFileId::new("second.mod3")),
        Some(second.binding().id()),
    );
    assert_eq!(
        routing.binding_ids(),
        [first.binding().id().clone(), second.binding().id().clone()]
            .into_iter()
            .collect(),
    );
}

/// 同一个文件被两个绑定声明必须拒绝，不能后写覆盖先写。
///
/// 这是族级随行文件的护栏：那些文件属于**包**、不属于任何槽位（`#349` 切片③b 的
/// 「族级随行文件提到包级」），一旦被塞进每个单元就会在这里撞上。两个 staging 根里
/// 都有它，取哪个都可能是错的，所以在组装阶段就拒绝，而不是提交时赌一把。
#[test]
fn routing_rejects_two_bindings_claiming_one_package_file() {
    let mut routing = RetargetSourceRouting::empty();
    routing
        .stage(
            PackageFileId::new("family/shared.epv"),
            ReplacementBindingId::parse("binding-first").expect("binding id"),
        )
        .expect("first claim");

    let error = routing
        .stage(
            PackageFileId::new("family/shared.epv"),
            ReplacementBindingId::parse("binding-second").expect("binding id"),
        )
        .expect_err("second claim on one package file");

    assert!(error.to_string().contains("family/shared.epv"));
    // 拒绝之后归属仍是先写的那一个——没有被覆盖。
    assert_eq!(
        routing.binding_for(&PackageFileId::new("family/shared.epv")),
        Some(&ReplacementBindingId::parse("binding-first").expect("binding id")),
    );
}

/// `merge` 与 `stage` 用同一道校验，合并路径也拦得住。
#[test]
fn merged_routing_rejects_a_package_file_claimed_by_both_plans() {
    let first = RetargetPlan::new(
        binding("binding-first", "one002"),
        source(),
        vec![action("shared.mod3", "one002", "mod3")],
        Vec::new(),
    )
    .expect("first plan");
    let second = RetargetPlan::new(
        binding("binding-second", "one003"),
        source(),
        vec![action("shared.mod3", "one003", "mod3")],
        Vec::new(),
    )
    .expect("second plan");

    let mut routing = first.source_routing();

    let error = routing
        .merge(second.source_routing())
        .expect_err("both plans claim shared.mod3");

    assert!(error.to_string().contains("shared.mod3"));
}

/// 空路由是有意义的一档：整个计划都不涉及 staging（未重定向安装、「保持原位」）。
#[test]
fn empty_routing_reports_itself_as_empty() {
    let routing = RetargetSourceRouting::empty();

    assert!(routing.is_empty());
    assert_eq!(routing.len(), 0);
    assert_eq!(routing.binding_for(&PackageFileId::new("anything")), None);
    assert!(routing.binding_ids().is_empty());
}
