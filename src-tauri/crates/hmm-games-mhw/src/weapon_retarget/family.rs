use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum WeaponFamilyError {
    #[error("unknown MHW weapon family")]
    UnknownFamily,
    #[error("invalid MHW weapon main id")]
    InvalidMainId,
    #[error("MHW weapon main id does not match its path family")]
    FamilyMismatch,
    #[error("unknown MHW weapon part")]
    UnknownPart,
}

impl WeaponFamilyError {
    pub fn code(self) -> &'static str {
        match self {
            Self::UnknownFamily => "weapon_unknown_family",
            Self::InvalidMainId | Self::FamilyMismatch => "weapon_invalid_main_id",
            Self::UnknownPart => "weapon_unknown_part",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeaponFamily {
    GreatSword,
    SwordAndShield,
    DualBlades,
    LongSword,
    Hammer,
    HuntingHorn,
    Lance,
    Gunlance,
    SwitchAxe,
    ChargeBlade,
    InsectGlaive,
    Bow,
    HeavyBowgun,
    LightBowgun,
}

impl WeaponFamily {
    pub const ALL: [Self; 14] = [
        Self::GreatSword,
        Self::SwordAndShield,
        Self::DualBlades,
        Self::LongSword,
        Self::Hammer,
        Self::HuntingHorn,
        Self::Lance,
        Self::Gunlance,
        Self::SwitchAxe,
        Self::ChargeBlade,
        Self::InsectGlaive,
        Self::Bow,
        Self::HeavyBowgun,
        Self::LightBowgun,
    ];

    pub fn parse(value: &str) -> Result<Self, WeaponFamilyError> {
        Self::from_token(value).ok_or(WeaponFamilyError::UnknownFamily)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::GreatSword => "two",
            Self::SwordAndShield => "one",
            Self::DualBlades => "sou",
            Self::LongSword => "swo",
            Self::Hammer => "ham",
            Self::HuntingHorn => "hue",
            Self::Lance => "lan",
            Self::Gunlance => "gun",
            Self::SwitchAxe => "saxe",
            Self::ChargeBlade => "caxe",
            Self::InsectGlaive => "rod",
            Self::Bow => "bow",
            Self::HeavyBowgun => "hbg",
            Self::LightBowgun => "lbg",
        }
    }

    pub fn path_family(self) -> &'static str {
        match self {
            Self::GreatSword => "wp/two",
            Self::SwordAndShield => "wp/one",
            Self::DualBlades => "wp/sou",
            Self::LongSword => "wp/swo",
            Self::Hammer => "wp/ham",
            Self::HuntingHorn => "wp/hue",
            Self::Lance => "wp/lan",
            Self::Gunlance => "wp/gun",
            Self::SwitchAxe => "wp/saxe",
            Self::ChargeBlade => "wp/caxe",
            Self::InsectGlaive => "wp/rod",
            Self::Bow => "wp/bow",
            Self::HeavyBowgun => "wp/hbg",
            Self::LightBowgun => "wp/lbg",
        }
    }

    pub fn secondary_part(self) -> Option<WeaponSecondaryPart> {
        match self {
            Self::SwordAndShield | Self::Lance | Self::Gunlance | Self::ChargeBlade => {
                Some(WeaponSecondaryPart::new(WeaponPartRole::Shield, "sld"))
            }
            Self::DualBlades => Some(WeaponSecondaryPart::new(WeaponPartRole::Right, "sou_r")),
            Self::LongSword => Some(WeaponSecondaryPart::new(WeaponPartRole::Sheath, "saya")),
            Self::GreatSword
            | Self::Hammer
            | Self::HuntingHorn
            | Self::SwitchAxe
            | Self::InsectGlaive
            | Self::Bow
            | Self::HeavyBowgun
            | Self::LightBowgun => None,
        }
    }

    fn from_token(value: &str) -> Option<Self> {
        match value {
            "two" => Some(Self::GreatSword),
            "one" => Some(Self::SwordAndShield),
            "sou" => Some(Self::DualBlades),
            "swo" => Some(Self::LongSword),
            "ham" => Some(Self::Hammer),
            "hue" => Some(Self::HuntingHorn),
            "lan" => Some(Self::Lance),
            "gun" => Some(Self::Gunlance),
            "saxe" => Some(Self::SwitchAxe),
            "caxe" => Some(Self::ChargeBlade),
            "rod" => Some(Self::InsectGlaive),
            "bow" => Some(Self::Bow),
            "hbg" => Some(Self::HeavyBowgun),
            "lbg" => Some(Self::LightBowgun),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeaponPartRole {
    Main,
    Shield,
    Right,
    Sheath,
}

impl WeaponPartRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Shield => "shield",
            Self::Right => "right",
            Self::Sheath => "sheath",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponSecondaryPart {
    role: WeaponPartRole,
    prefix: &'static str,
}

impl WeaponSecondaryPart {
    const fn new(role: WeaponPartRole, prefix: &'static str) -> Self {
        Self { role, prefix }
    }

    pub fn role(self) -> WeaponPartRole {
        self.role
    }

    pub fn prefix(self) -> &'static str {
        self.prefix
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WeaponMainId {
    canonical: String,
    family: WeaponFamily,
    number: u16,
    has_bs_prefix: bool,
}

impl WeaponMainId {
    pub fn parse(value: &str) -> Result<Self, WeaponFamilyError> {
        let (has_bs_prefix, body) = match value.strip_prefix("bs_") {
            Some(body) => (true, body),
            None => (false, value),
        };
        if body.len() < 4 {
            return Err(WeaponFamilyError::InvalidMainId);
        }

        let split_at = body.len() - 3;
        let (family_token, digits) = body.split_at(split_at);
        if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(WeaponFamilyError::InvalidMainId);
        }
        let family =
            WeaponFamily::from_token(family_token).ok_or(WeaponFamilyError::InvalidMainId)?;
        let number = digits
            .parse::<u16>()
            .map_err(|_| WeaponFamilyError::InvalidMainId)?;

        Ok(Self {
            canonical: value.to_owned(),
            family,
            number,
            has_bs_prefix,
        })
    }

    pub fn parse_for_family(
        value: &str,
        expected_family: WeaponFamily,
    ) -> Result<Self, WeaponFamilyError> {
        let parsed = Self::parse(value)?;
        if parsed.family != expected_family {
            return Err(WeaponFamilyError::FamilyMismatch);
        }
        Ok(parsed)
    }

    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    pub fn family(&self) -> WeaponFamily {
        self.family
    }

    pub fn number(&self) -> u16 {
        self.number
    }

    pub fn has_bs_prefix(&self) -> bool {
        self.has_bs_prefix
    }

    pub fn part_for_role(&self, role: WeaponPartRole) -> Result<WeaponPartId, WeaponFamilyError> {
        let prefix = match role {
            WeaponPartRole::Main => self.family.as_str(),
            _ => self
                .family
                .secondary_part()
                .filter(|part| part.role() == role)
                .map(WeaponSecondaryPart::prefix)
                .ok_or(WeaponFamilyError::UnknownPart)?,
        };
        let bs_prefix = if self.has_bs_prefix { "bs_" } else { "" };
        Ok(WeaponPartId {
            canonical: format!("{bs_prefix}{prefix}{:03}", self.number),
            family: self.family,
            role,
            number: self.number,
            has_bs_prefix: self.has_bs_prefix,
            variant_suffix: String::new(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WeaponPartId {
    canonical: String,
    family: WeaponFamily,
    role: WeaponPartRole,
    number: u16,
    has_bs_prefix: bool,
    /// 已登记部件 ID 之后作者自加的变体后缀，如 `saya035ol` 的 `ol`（#336 ②b）。
    ///
    /// 它标识**另一个模型**，不是同一个部件的别名：`saya035` 与 `saya035ol` 各自成对、
    /// 各自改名。后缀必须逐字带到目标部件上，否则两者会双双改成 `saya019` 而互相覆盖。
    variant_suffix: String,
}

impl WeaponPartId {
    pub fn parse_for_main(value: &str, main_id: &WeaponMainId) -> Result<Self, WeaponFamilyError> {
        let mut candidates = vec![main_id.part_for_role(WeaponPartRole::Main)?];
        if let Some(secondary) = main_id.family().secondary_part() {
            candidates.push(main_id.part_for_role(secondary.role())?);
        }

        if let Some(exact) = candidates.iter().find(|part| value == part.as_str()) {
            return Ok(exact.clone());
        }

        /*
         * 变体后缀（#336 ②b）。泡狐太刀的 `saya035ol` 过去在这里判 `UnknownPart`，
         * 而 `analysis.rs` 把它升级成否决整包——库里唯一一把太刀 Mod 因此不可重定向。
         *
         * 它本质上不是「没见过的部件」：前缀就是本族已登记的鞘 `saya035`，作者只是加了
         * 个变体标记。`part_rename::rename_part_prefix` **本来就能算对** `saya035ol` →
         * `saya019ol`，挡住它的只有这里的全等比较。所以放宽的是识别，不是改名规则。
         *
         * 两条守卫与 `rename_part_prefix` 逐字相同，理由也相同——两处必须对同一个文件名
         * 得出同一个结论，否则磁盘改名与 MRL3 引用改写会分叉：
         *
         * 守卫① 后缀不能以数字开头。部件 ID 是 `<prefix><3 位数字>`，否则 `saya0351`
         *       会被读成 `saya035` + 变体 `1`。
         * 守卫② 后缀里不能再出现任何部件 ID。`swo035saya035` 无法判断作者意图，
         *       且改名后内层的 `saya035` 不会被改写，会产生断链。
         */
        let matched = candidates
            .iter()
            .filter(|part| value.starts_with(part.as_str()))
            // 同族部件 ID 可能互为前缀，取最长命中。
            .max_by_key(|part| part.as_str().len());
        let Some(part) = matched else {
            return Err(WeaponFamilyError::UnknownPart);
        };
        let suffix = &value[part.as_str().len()..];

        if suffix.is_empty()
            || suffix.as_bytes()[0].is_ascii_digit()
            || candidates
                .iter()
                .any(|candidate| suffix.contains(candidate.as_str()))
        {
            return Err(WeaponFamilyError::UnknownPart);
        }
        Ok(part.clone().with_variant_suffix(suffix))
    }

    /// 把变体后缀接到部件 ID 上。目标部件由 `part_for_role` 产出（不带后缀），
    /// 重定向时用本方法把源部件的后缀原样接过去。
    pub(super) fn with_variant_suffix(mut self, suffix: &str) -> Self {
        self.canonical.push_str(suffix);
        self.variant_suffix = suffix.to_owned();
        self
    }

    pub fn variant_suffix(&self) -> &str {
        &self.variant_suffix
    }

    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    pub fn family(&self) -> WeaponFamily {
        self.family
    }

    pub fn role(&self) -> WeaponPartRole {
        self.role
    }

    pub fn number(&self) -> u16 {
        self.number
    }

    pub fn has_bs_prefix(&self) -> bool {
        self.has_bs_prefix
    }
}
