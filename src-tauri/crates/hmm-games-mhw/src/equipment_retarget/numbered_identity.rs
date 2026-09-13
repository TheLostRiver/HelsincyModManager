use crate::{KinsectId, KinsectResourceRoot, WeaponFamily, WeaponMainId, WeaponResourceRoot};
use hmm_core::InstallTargetPath;

#[derive(Clone, Copy)]
pub(super) enum NumberedId<'a> {
    Weapon(&'a WeaponMainId),
    Kinsect(&'a KinsectId),
}

impl<'a> NumberedId<'a> {
    pub fn as_str(self) -> &'a str {
        match self {
            Self::Weapon(id) => id.as_str(),
            Self::Kinsect(id) => id.as_str(),
        }
    }

    pub fn number(self) -> u16 {
        match self {
            Self::Weapon(id) => id.number(),
            Self::Kinsect(id) => id.number(),
        }
    }

    pub fn has_bs_prefix(self) -> bool {
        matches!(self, Self::Weapon(id) if id.has_bs_prefix())
    }

    pub fn parts(self) -> (u16, bool) {
        (self.number(), self.has_bs_prefix())
    }

    pub fn family(self) -> &'static str {
        match self {
            Self::Weapon(id) => id.family().as_str(),
            Self::Kinsect(_) => "mus",
        }
    }

    pub fn secondary_prefix(self) -> Option<&'static str> {
        match self {
            Self::Weapon(id) => id.family().secondary_part().map(|part| part.prefix()),
            Self::Kinsect(_) => None,
        }
    }

    pub fn conflicts_with_prefix(self, prefix: &str) -> bool {
        let normalized = prefix.to_ascii_lowercase();
        (normalized == "mus" || WeaponFamily::parse(&normalized).is_ok())
            && normalized != self.family()
    }

    pub fn conflicts_with_main(self, name: &str) -> bool {
        if !name.is_ascii() {
            return false;
        }
        let normalized = name.to_ascii_lowercase();
        (WeaponMainId::parse(&normalized).is_ok() || KinsectId::parse(&normalized).is_ok())
            && normalized != self.as_str()
    }
}

#[derive(Clone, Copy)]
pub(super) enum NumberedRoot<'a> {
    Weapon(&'a WeaponResourceRoot),
    Kinsect(&'a KinsectResourceRoot),
}

impl<'a> NumberedRoot<'a> {
    pub fn main_id(self) -> NumberedId<'a> {
        match self {
            Self::Weapon(root) => NumberedId::Weapon(root.main_id()),
            Self::Kinsect(root) => NumberedId::Kinsect(root.id()),
        }
    }

    pub fn contains(self, path: &InstallTargetPath) -> bool {
        match self {
            Self::Weapon(root) => root.contains(path),
            Self::Kinsect(root) => root.contains(path),
        }
    }
}
