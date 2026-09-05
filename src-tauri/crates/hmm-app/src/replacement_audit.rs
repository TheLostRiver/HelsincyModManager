use std::collections::BTreeMap;

use hmm_core::{ContentTransformerIdentity, ReplacementAdapterFacts, ReplacementBindingSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementAdapterAuditFacts {
    adapter_id: String,
    strategy_id: String,
    strategy_version: u32,
    transformer_identities: Vec<ContentTransformerIdentity>,
    part_count: u32,
    file_count: u32,
    /// #336 切片③：适配器主动丢弃、未进入计划的包内文件数（可执行 / 脚本拒绝清单）。
    /// 只带计数——被排除的路径是第三方 Mod 内容，不进 Audit Log。
    excluded_file_count: u32,
}

impl ReplacementAdapterAuditFacts {
    pub fn from_adapter_facts(facts: &ReplacementAdapterFacts) -> Option<Self> {
        (!facts.transformer_identities().is_empty()).then(|| Self {
            adapter_id: facts.adapter_id().to_owned(),
            strategy_id: facts.strategy_id().to_owned(),
            strategy_version: facts.strategy_version(),
            transformer_identities: facts.transformer_identities().to_vec(),
            part_count: facts.part_count(),
            file_count: facts.file_count(),
            excluded_file_count: facts.excluded_file_count(),
        })
    }
}

pub(crate) fn unique_adapter_audit_facts(
    bindings: &[ReplacementBindingSnapshot],
) -> Option<Box<ReplacementAdapterAuditFacts>> {
    let mut facts = bindings
        .iter()
        .filter_map(|binding| binding.adapter_facts());
    let first = facts.next()?;
    if !facts.all(|candidate| candidate == first) {
        return None;
    }
    ReplacementAdapterAuditFacts::from_adapter_facts(first).map(Box::new)
}

pub(crate) fn append_adapter_audit_fields(
    fields: &mut BTreeMap<String, String>,
    facts: Option<&ReplacementAdapterAuditFacts>,
) {
    let Some(facts) = facts else {
        return;
    };
    fields.insert("adapter_id".to_owned(), facts.adapter_id.clone());
    fields.insert("strategy_id".to_owned(), facts.strategy_id.clone());
    fields.insert(
        "strategy_version".to_owned(),
        facts.strategy_version.to_string(),
    );
    fields.insert("part_count".to_owned(), facts.part_count.to_string());
    fields.insert("file_count".to_owned(), facts.file_count.to_string());
    // 恒定写入而不是「非零才写」：审计要能区分「本次没有可拒绝的文件」和
    // 「这条记录来自还没有拒绝清单的版本」，缺字段与 0 不是同一件事。
    fields.insert(
        "excluded_file_count".to_owned(),
        facts.excluded_file_count.to_string(),
    );
    fields.insert(
        "transformer_count".to_owned(),
        facts.transformer_identities.len().to_string(),
    );

    if let [identity] = facts.transformer_identities.as_slice() {
        fields.insert(
            "transformer_id".to_owned(),
            identity.transformer_id().to_owned(),
        );
        fields.insert(
            "transformer_version".to_owned(),
            identity.transformer_version().to_string(),
        );
        return;
    }

    for (index, identity) in facts.transformer_identities.iter().enumerate() {
        fields.insert(
            format!("transformer_{index}_id"),
            identity.transformer_id().to_owned(),
        );
        fields.insert(
            format!("transformer_{index}_version"),
            identity.transformer_version().to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        byte.to_string().repeat(64)
    }

    fn facts_with_exclusions(excluded_file_count: u32) -> ReplacementAdapterFacts {
        ReplacementAdapterFacts::new(
            1,
            "mhw.weapon",
            "mrl3-texture-path",
            3,
            digest('a'),
            digest('b'),
            digest('c'),
        )
        .expect("adapter facts")
        .with_transformers(
            vec![
                ContentTransformerIdentity::new("mhw.weapon.mrl3-texture-path.v1", 1)
                    .expect("transformer identity"),
            ],
            1,
            15,
        )
        .expect("transformer facts")
        .with_excluded_file_count(excluded_file_count)
    }

    /// #336 切片③ 的审计留痕：被拒绝清单丢弃的文件数必须进 Audit Log。
    /// 没有它，「装完之后包里少了一个文件」在事后无从追溯。
    #[test]
    fn excluded_file_count_reaches_the_audit_fields() {
        let audit = ReplacementAdapterAuditFacts::from_adapter_facts(&facts_with_exclusions(2))
            .expect("audit facts");
        let mut fields = BTreeMap::new();
        append_adapter_audit_fields(&mut fields, Some(&audit));

        assert_eq!(
            fields.get("excluded_file_count").map(String::as_str),
            Some("2")
        );
        assert_eq!(fields.get("file_count").map(String::as_str), Some("15"));
    }

    /// 没有被排除的文件时字段仍要写出 `0`——缺字段与 `0` 不是同一件事：
    /// 前者说明记录来自还没有拒绝清单的版本，后者说明本次确实没有可拒绝的文件。
    #[test]
    fn a_zero_exclusion_count_is_still_written_out() {
        let audit = ReplacementAdapterAuditFacts::from_adapter_facts(&facts_with_exclusions(0))
            .expect("audit facts");
        let mut fields = BTreeMap::new();
        append_adapter_audit_fields(&mut fields, Some(&audit));

        assert_eq!(
            fields.get("excluded_file_count").map(String::as_str),
            Some("0")
        );
    }

    /// 审计只出计数。被排除的路径是第三方 Mod 内容，`SECURITY.md` 禁止它进日志。
    #[test]
    fn audit_fields_carry_no_paths_or_package_content() {
        let audit = ReplacementAdapterAuditFacts::from_adapter_facts(&facts_with_exclusions(2))
            .expect("audit facts");
        let mut fields = BTreeMap::new();
        append_adapter_audit_fields(&mut fields, Some(&audit));

        // transformer id 里合法地含点（`mhw.weapon.mrl3-texture-path.v1`），
        // 所以判据是路径分隔符而不是点。
        for value in fields.values() {
            assert!(
                !value.contains('/') && !value.contains('\\'),
                "审计字段不得包含路径：{value}"
            );
        }
    }
}
