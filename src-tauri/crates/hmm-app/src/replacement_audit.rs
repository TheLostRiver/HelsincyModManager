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
