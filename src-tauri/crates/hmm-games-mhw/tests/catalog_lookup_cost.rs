//! catalog 查找的**代价**回归。
//!
//! # 为什么这条必须存在
//!
//! `replacement_catalog()` 从接通那天（`c004d62`，2026-08-08）起就没有缓存，每次调用都要把
//! `include_str!` 进来的静态 JSON 重新解析：先按 envelope 解一遍、再按完整结构解一遍，然后
//! 全量校验。数据小的时候没人察觉；WR-02B（2026-08-21）把武器目标做到 601 条 678 KiB、
//! `#356`（2026-09-07）又把防具从 269 扩到 529 加 279 KiB 之后，debug 构建下实测单次
//! **217ms**——而 `find_replacement_target` 就在「逐槽位」的循环里，于是「打开 Mod 库卡死
//! 几十秒」「每次安装／卸载／导入都卡住」。
//!
//! 这是一条**看不见**的回归：功能全对、测试全绿，只是慢到不能用。所以用代价本身当判据。
//!
//! # 为什么不怕 flaky
//!
//! 判据与实际值差三个数量级：修好之后逐次查找是微秒级，1000 次远低于 2 秒；一旦缓存被去掉，
//! 1000 次就是 200 秒以上。中间没有含糊地带，慢机器或 CI 抖动都跨不过这个差距。

use hmm_ports::ReplacementCatalogProvider;
use std::time::{Duration, Instant};

/// 逐槽位查找不得重新解析 catalog。
#[test]
fn repeated_target_lookups_do_not_reparse_the_catalog() {
    let catalog_provider = hmm_games_mhw::MhwReplacementCatalog;

    // 第一次调用允许解析（也只该有这一次）。
    let catalog = catalog_provider
        .replacement_catalog()
        .expect("catalog should load");
    let target_id = catalog
        .targets()
        .first()
        .expect("catalog should not be empty")
        .id()
        .clone();

    let started = Instant::now();
    for _ in 0..1_000 {
        catalog_provider
            .find_replacement_target(&target_id)
            .expect("target should resolve");
    }
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "1000 次目标查找用了 {elapsed:?}——catalog 很可能又在每次查找时重新解析了。\
         查找必须走借用（borrow_full_catalog），不能走返回 owned 的 replacement_catalog()。"
    );
}

/// 防具侧同理：它自己也是一个独立的 provider。
#[test]
fn repeated_armor_target_lookups_do_not_reparse_the_catalog() {
    let catalog_provider = hmm_games_mhw::MhwArmorCatalog;
    let catalog = catalog_provider
        .replacement_catalog()
        .expect("catalog should load");
    let target_id = catalog
        .targets()
        .first()
        .expect("catalog should not be empty")
        .id()
        .clone();

    let started = Instant::now();
    for _ in 0..1_000 {
        catalog_provider
            .find_replacement_target(&target_id)
            .expect("target should resolve");
    }
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "1000 次防具目标查找用了 {elapsed:?}——防具 catalog 很可能又在每次查找时重新解析了。"
    );
}

/// 解析本身只该发生一次：反复要 owned catalog 也不该再解析，只该是克隆。
#[test]
fn repeated_owned_catalog_requests_only_clone() {
    let catalog_provider = hmm_games_mhw::MhwReplacementCatalog;
    catalog_provider
        .replacement_catalog()
        .expect("catalog should load");

    let started = Instant::now();
    for _ in 0..100 {
        catalog_provider
            .replacement_catalog()
            .expect("catalog should load");
    }
    let elapsed = started.elapsed();

    // 克隆 1000+ 个三语目标本身不便宜（debug 下约 11ms），所以这里的预算比查找宽得多；
    // 但重新解析是 217ms／次，100 次就是 20 秒以上，照样跨不过去。
    assert!(
        elapsed < Duration::from_secs(5),
        "100 次取 owned catalog 用了 {elapsed:?}——静态数据不该被反复解析。"
    );
}
