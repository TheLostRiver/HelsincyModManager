use hmm_core::PluginSelectionSnapshot;
use sha2::{Digest, Sha256};

/// 空集合不改变旧摘要；新增选择必须同时覆盖身份、路径、字节摘要和包含／跳过意图。
pub(crate) fn hash_selections(hash: &mut Sha256, selections: &[PluginSelectionSnapshot]) {
    if selections.is_empty() {
        return;
    }
    hash.update(b"hmm-plugin-selections-v1");
    let mut ordered = selections.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.scope()
            .mod_id
            .as_str()
            .cmp(right.scope().mod_id.as_str())
    });
    for selection in ordered {
        let bytes = serde_json::to_vec(selection).expect("plugin selection facts are serializable");
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
}
