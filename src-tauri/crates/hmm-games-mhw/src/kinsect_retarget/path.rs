use hmm_core::InstallTargetPath;
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("invalid MHW kinsect resource path")]
pub struct KinsectPathError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KinsectId {
    value: String,
    number: u16,
}

impl KinsectId {
    pub fn parse(value: &str) -> Result<Self, KinsectPathError> {
        let digits = value.strip_prefix("mus").ok_or(KinsectPathError)?;
        if digits.len() != 3 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(KinsectPathError);
        }
        Ok(Self {
            value: value.to_owned(),
            number: digits.parse().map_err(|_| KinsectPathError)?,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn number(&self) -> u16 {
        self.number
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KinsectResourceRoot {
    path: InstallTargetPath,
    id: KinsectId,
}

impl KinsectResourceRoot {
    pub fn parse(value: &str) -> Result<Self, KinsectPathError> {
        let path = InstallTargetPath::parse(value, ["nativePC"]).map_err(|_| KinsectPathError)?;
        let parts = path.as_str().split('/').collect::<Vec<_>>();
        if parts.len() != 4 || parts[..3] != ["nativePC", "wp", "mus"] {
            return Err(KinsectPathError);
        }
        let id = KinsectId::parse(parts[3])?;
        Ok(Self { path, id })
    }

    pub fn of_resource_path(value: &str) -> Option<Self> {
        // 先验证完整路径，不能把不安全尾部截掉后当成合法来源。
        let path = InstallTargetPath::parse(value, ["nativePC"]).ok()?;
        let parts = path.as_str().split('/').collect::<Vec<_>>();
        (parts.len() > 4)
            .then(|| Self::parse(&parts[..4].join("/")).ok())
            .flatten()
    }

    pub fn normalized_path(&self) -> &InstallTargetPath {
        &self.path
    }

    pub fn id(&self) -> &KinsectId {
        &self.id
    }

    pub fn path_family(&self) -> &'static str {
        "wp/mus"
    }

    pub fn contains(&self, path: &InstallTargetPath) -> bool {
        path.as_str()
            .starts_with(&format!("{}/", self.path.as_str()))
    }
}
