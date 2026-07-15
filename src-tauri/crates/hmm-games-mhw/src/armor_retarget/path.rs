use hmm_core::{InstallTargetPath, InstallTargetPathError};
use thiserror::Error;

const ARMOR_PATH_SEGMENT_COUNT: usize = 7;
const NATIVE_PC_ROOT: &str = "nativePC";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ArmorPathError {
    #[error("not an MHW armor resource path")]
    NotArmorPath,
    #[error("malformed MHW armor resource path")]
    MalformedArmorPath,
    #[error("invalid MHW armor slot")]
    InvalidSlot,
    #[error("unsafe MHW armor resource path")]
    UnsafePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArmorEquipFamily {
    Female,
    Male,
}

impl ArmorEquipFamily {
    fn from_segment(segment: &str) -> Option<Self> {
        match segment {
            "f_equip" => Some(Self::Female),
            "m_equip" => Some(Self::Male),
            _ => None,
        }
    }

    fn path_family(self) -> &'static str {
        match self {
            Self::Female => "pl/f_equip",
            Self::Male => "pl/m_equip",
        }
    }

    fn is_supported(self) -> bool {
        self == Self::Female
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmorResourcePath {
    normalized_path: InstallTargetPath,
    segments: [String; ARMOR_PATH_SEGMENT_COUNT],
    family: ArmorEquipFamily,
}

impl ArmorResourcePath {
    pub fn parse(value: &str) -> Result<Self, ArmorPathError> {
        let normalized_path = match InstallTargetPath::parse(value, [NATIVE_PC_ROOT]) {
            Ok(path) => path,
            Err(InstallTargetPathError::TargetRootNotAllowed { .. }) => {
                return Err(ArmorPathError::NotArmorPath)
            }
            Err(_) => return Err(ArmorPathError::UnsafePath),
        };

        let parts = normalized_path.as_str().split('/').collect::<Vec<_>>();
        let is_armor_candidate = parts.first() == Some(&NATIVE_PC_ROOT)
            && parts.get(1) == Some(&"pl")
            && parts
                .get(2)
                .and_then(|segment| ArmorEquipFamily::from_segment(segment))
                .is_some();

        if !is_armor_candidate {
            return Err(ArmorPathError::NotArmorPath);
        }
        if parts.len() != ARMOR_PATH_SEGMENT_COUNT || parts[4] != "arm" || parts[5] != "mod" {
            return Err(ArmorPathError::MalformedArmorPath);
        }
        if !is_valid_armor_slot(parts[3]) {
            return Err(ArmorPathError::InvalidSlot);
        }

        let family = ArmorEquipFamily::from_segment(parts[2])
            .expect("armor candidates always have a known equip family");
        let segments = parts
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
            .try_into()
            .expect("armor path segment count was checked");

        Ok(Self {
            normalized_path,
            segments,
            family,
        })
    }

    pub fn normalized_path(&self) -> &InstallTargetPath {
        &self.normalized_path
    }

    pub fn slot(&self) -> &str {
        &self.segments[3]
    }

    pub fn path_family(&self) -> &'static str {
        self.family.path_family()
    }

    pub fn is_supported(&self) -> bool {
        self.family.is_supported()
    }

    pub fn retarget(&self, target_slot: &str) -> Result<InstallTargetPath, ArmorPathError> {
        if !is_valid_armor_slot(target_slot) {
            return Err(ArmorPathError::InvalidSlot);
        }

        let mut target_segments = self.segments.clone();
        target_segments[3] = target_slot.to_owned();
        InstallTargetPath::parse(target_segments.join("/"), [NATIVE_PC_ROOT])
            .map_err(|_| ArmorPathError::UnsafePath)
    }
}

pub(crate) fn is_valid_armor_slot(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && &bytes[..2] == b"pl"
        && bytes[2..5].iter().all(u8::is_ascii_digit)
        && bytes[5] == b'_'
        && bytes[6..].iter().all(u8::is_ascii_digit)
}
