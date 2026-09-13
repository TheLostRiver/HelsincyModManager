use hmm_core::{GameId, InstallTargetPath, PluginSelectionScope, PluginSelectionSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginFileCheck {
    Supported,
    InvalidFormat,
    UnsupportedArchitecture,
    NotDynamicLibrary,
    PolicyExcluded,
}

impl PluginFileCheck {
    pub fn code(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::InvalidFormat => "invalid_format",
            Self::UnsupportedArchitecture => "unsupported_architecture",
            Self::NotDynamicLibrary => "not_dynamic_library",
            Self::PolicyExcluded => "policy_excluded",
        }
    }
}

/// 纯检查接口，不加载 DLL、不调用平台执行器，也不向前端暴露文件系统能力。
pub trait GamePluginPolicy: Send + Sync {
    fn game_id(&self) -> GameId;
    fn policy_id(&self) -> &'static str;
    fn policy_version(&self) -> u32;
    fn is_candidate(&self, target: &InstallTargetPath) -> bool;
    fn is_excluded_attachment(&self, _target: &InstallTargetPath) -> bool {
        false
    }
    fn inspect(&self, bytes: &[u8]) -> PluginFileCheck;
}

/// 用户的待应用选择意图；已应用结果另随 InstallManifest 保存。
pub trait PluginSelectionRepository: Send + Sync {
    fn load_selection(
        &self,
        scope: &PluginSelectionScope,
    ) -> anyhow::Result<Option<PluginSelectionSnapshot>>;
    fn save_selection(&self, selection: &PluginSelectionSnapshot) -> anyhow::Result<()>;
}
