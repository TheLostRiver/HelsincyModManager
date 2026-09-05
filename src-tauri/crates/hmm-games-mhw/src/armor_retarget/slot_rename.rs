//! 防具侧重定向的改写规则：**在源槽位子树内替换槽位编号段**。
//!
//! # 为什么是结构规则而不是词表
//!
//! 早期实现要求路径形如 `<slot>/arm/mod/<file>`——把「部位」写死成 `arm` 一个词，
//! 于是真实套装的 `body` `helm` `leg` `wst` 四个部位全部判成「路径畸形」并否决整包
//! （#342 的 A/B）。补成五个词同样是死路：任何作者自建的子目录、任何没见过的扩展名
//! 都会再次触发同一类失败，而 Mod 作者的目录结构不可能被穷举。
//!
//! 正确的规则不需要任何词表：**只要命中槽位编号段 `<3位>_<4位>` 就替换**，对源槽位目录下
//! 的一切一视同仁——不认识部位、不认识扩展名、没有注册表。因为它不依赖任何预先记录的知识，
//! 新 Mod 的任何目录布局都不会让它失效。
//!
//! # 规则
//!
//! ```text
//! 槽位令牌 = 槽位目录名去掉 pl 前缀        pl078_0000 → 078_0000
//! 作用范围 = nativePC/pl/<equip>/<slot>/ 之下的一切，任意深度、任意扩展名
//! 改写     = 在该文件的整条相对路径上，把源令牌的每一处出现换成目标令牌
//! 其余     = 一个字节都不碰
//! ```
//!
//! 在整条路径上替换，一次同时解决槽位目录段与文件名段——真机实验 A（重定向前后全量快照，
//! 逐文件比对 path + size + SHA-256）观测到的正是
//! `pl078_0000/arm/mod/f_arm078_0000.mod3` → `pl123_0000/arm/mod/f_arm123_0000.mod3`
//! （哈希证明字节完全相同，只有路径变了）。嵌套子目录里若也带编号，同样跟着改。
//!
//! **与武器侧不可复用同一函数**（#336 洞见 2 明文）：武器侧是「文件名开头的部件 ID 前缀
//! 替换」，且存在 `bs_two012` → `two020` 这种连前缀一起换掉的情况，不是纯数字替换。

use crate::package_path::NATIVE_PC_ROOT;
use hmm_core::InstallTargetPath;

/// 槽位令牌 = 槽位目录名去掉 `pl` 前缀，形如 `078_0000`。
///
/// 槽位语法由 [`super::path::is_valid_armor_slot`] 保证（`pl` + 3 位 + `_` + 4 位），
/// 所以令牌一定是 8 个字符的 `<3位>_<4位>`——足够独特，在一个槽位子树内直接全量替换
/// 不需要再加边界守卫。
pub(super) fn slot_token(slot: &str) -> Option<&str> {
    slot.strip_prefix("pl")
}

/// 把源槽位子树内某个文件的相对路径改写到目标槽位。
///
/// 调用方必须先确认 `path` 确实落在源槽位目录之内；本函数不重复判定归属，只做替换与
/// 结果复验。
///
/// 返回 `None` 表示改写结果不再是一条安全的 `nativePC` 相对路径——那是结构性异常，
/// 调用方应失败关闭而不是安装一条来路不明的路径。
pub(super) fn retarget_within_slot(
    path: &InstallTargetPath,
    source_token: &str,
    target_token: &str,
) -> Option<InstallTargetPath> {
    let rewritten = path.as_str().replace(source_token, target_token);
    // 令牌是 `<3位>_<4位>`，替换不可能引入分隔符或 `..`；这里仍然重新走一遍完整校验，
    // 因为「改写产出的路径必须自证安全」比「推理它一定安全」可靠。
    InstallTargetPath::parse(rewritten, [NATIVE_PC_ROOT]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> InstallTargetPath {
        InstallTargetPath::parse(value, [NATIVE_PC_ROOT]).expect("fixture path")
    }

    #[test]
    fn slot_tokens_drop_the_pl_prefix() {
        assert_eq!(slot_token("pl078_0000"), Some("078_0000"));
        assert_eq!(slot_token("pl123_0000"), Some("123_0000"));
    }

    #[test]
    fn the_reference_implementations_rename_is_reproduced_exactly() {
        /*
         * 真机实验 A 的实测样例，五个部位各一条。注意目录段与文件名段**同时**被改写——
         * 旧实现只改目录段，装出来是「目录对、文件名错」。
         */
        for (source, expected) in [
            (
                "nativePC/pl/f_equip/pl078_0000/arm/mod/f_arm078_0000.mod3",
                "nativePC/pl/f_equip/pl123_0000/arm/mod/f_arm123_0000.mod3",
            ),
            (
                "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.mrl3",
                "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.mrl3",
            ),
            (
                "nativePC/pl/f_equip/pl078_0000/helm/mod/f_helm078_0000.evhl",
                "nativePC/pl/f_equip/pl123_0000/helm/mod/f_helm123_0000.evhl",
            ),
            (
                "nativePC/pl/f_equip/pl078_0000/leg/mod/f_leg078_0000.ccl",
                "nativePC/pl/f_equip/pl123_0000/leg/mod/f_leg123_0000.ccl",
            ),
            (
                "nativePC/pl/f_equip/pl078_0000/wst/mod/f_wst078_0000.ctc",
                "nativePC/pl/f_equip/pl123_0000/wst/mod/f_wst123_0000.ctc",
            ),
        ] {
            assert_eq!(
                retarget_within_slot(&path(source), "078_0000", "123_0000")
                    .expect("rewritten path")
                    .as_str(),
                expected
            );
        }
    }

    #[test]
    fn unknown_part_directories_and_extensions_are_carried_along_unchanged_in_shape() {
        /*
         * #342 的核心：规则**不认识部位、不认识扩展名**。这里全部是词表里不存在的形态——
         * 作者自造的部位目录、没见过的扩展名、嵌套两层的子目录、名字里不含编号的文件。
         * 每一条都必须照常落到目标槽位，不能有任何一条触发失败。
         */
        for (source, expected) in [
            // 词表里没有的部位目录
            (
                "nativePC/pl/f_equip/pl078_0000/cloak/mod/f_cloak078_0000.mod3",
                "nativePC/pl/f_equip/pl123_0000/cloak/mod/f_cloak123_0000.mod3",
            ),
            // 没见过的扩展名
            (
                "nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000.whatever",
                "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000.whatever",
            ),
            // 嵌套更深的作者子目录
            (
                "nativePC/pl/f_equip/pl078_0000/body/mod/tex/custom/skin_BM.tex",
                "nativePC/pl/f_equip/pl123_0000/body/mod/tex/custom/skin_BM.tex",
            ),
            // 文件名不含编号：只有目录段变
            (
                "nativePC/pl/f_equip/pl078_0000/body/mod/readme_from_author.txt",
                "nativePC/pl/f_equip/pl123_0000/body/mod/readme_from_author.txt",
            ),
            // 中间目录也带编号：一起改
            (
                "nativePC/pl/f_equip/pl078_0000/extra078_0000/f_x078_0000.tex",
                "nativePC/pl/f_equip/pl123_0000/extra123_0000/f_x123_0000.tex",
            ),
            // 直接挂在槽位根下，没有部位段
            (
                "nativePC/pl/f_equip/pl078_0000/f_078_0000.ctc",
                "nativePC/pl/f_equip/pl123_0000/f_123_0000.ctc",
            ),
        ] {
            assert_eq!(
                retarget_within_slot(&path(source), "078_0000", "123_0000")
                    .expect("规则不得因为没见过这种形态就失败")
                    .as_str(),
                expected
            );
        }
    }

    #[test]
    fn the_token_is_replaced_everywhere_it_occurs_rather_than_being_called_ambiguous() {
        /*
         * 同一条路径里编号出现多次时**全量替换**，不判「有歧义」然后拒绝：出现多次说明
         * 它们都是这个槽位的编号，全改才是对的。判歧义只会让本来能装的包装不了——这正是
         * #342 要消灭的那种「形态不认识就拒绝」。
         */
        assert_eq!(
            retarget_within_slot(
                &path("nativePC/pl/f_equip/pl078_0000/body/mod/f_body078_0000_078_0000.mod3"),
                "078_0000",
                "123_0000",
            )
            .expect("rewritten path")
            .as_str(),
            "nativePC/pl/f_equip/pl123_0000/body/mod/f_body123_0000_123_0000.mod3"
        );
    }
}
