use anyhow::Result;
use hmm_core::{ModId, ProfileId, ReplacementBindingSnapshot};

/// 玩家在替换目标面板发起 retarget 安装时表达的目标选择意图（持久化）。
///
/// 选择意图让批量/标准安装能够 fail closed：持有未完成选择意图的 Mod 不允许
/// 走普通安装——普通安装会把未重定向的原始 Mod 装进源路径，绑定记录与实际
/// 写入不符。持有意图的 Mod 必须从替换目标面板完成安装。意图在 retarget
/// 安装完成后清除；安装失败/取消时保留，引导用户回到面板重试。
pub trait ReplacementSelectionRepository: Send + Sync {
    fn load_selection(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<Option<ReplacementBindingSnapshot>>;
    fn save_selection(&self, binding: &ReplacementBindingSnapshot) -> Result<()>;
    fn remove_selection(&self, profile_id: &ProfileId, mod_id: &ModId) -> Result<()>;
}
