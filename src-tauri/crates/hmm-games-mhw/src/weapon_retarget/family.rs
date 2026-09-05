use super::part_rename::split_weapon_stem;
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

    /// 部件前缀对应的 role。
    ///
    /// #343 起 role **不再参与改名**——改名只看槽位数字，见 [`super::part_rename`]。role
    /// 只剩两个用途：`PartialPartSet` 告警（主件或本族默认副件缺失）与计划里的稳定排序。
    /// 因此没登记过的前缀归入 `Auxiliary` 即可，不需要为它扩表，也不再是失败理由。
    fn role_for_prefix(self, prefix: &str) -> WeaponPartRole {
        if prefix.eq_ignore_ascii_case(self.as_str()) {
            return WeaponPartRole::Main;
        }
        match self.secondary_part() {
            Some(secondary) if prefix.eq_ignore_ascii_case(secondary.prefix()) => secondary.role(),
            _ => WeaponPartRole::Auxiliary,
        }
    }

    /// 本族的**默认**副件。仅用于 `PartialPartSet` 告警；不是可接受部件的完整清单，
    /// 返回 `None` 只表示「本族没有默认副件」，不表示「本族不能有副件」。
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
    /// 前缀不是主件、也不是本族默认副件的模型（#343）。**不是错误**——真实包里这类模型
    /// 很常见，改名规则不需要认识它。
    Auxiliary,
}

impl WeaponPartRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Shield => "shield",
            Self::Right => "right",
            Self::Sheath => "sheath",
            Self::Auxiliary => "auxiliary",
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
}

impl WeaponPartId {
    /// 结构化识别一个部件 ID（#343）。
    ///
    /// 判据只有「主干形如 `<bs_?><前缀><本槽位 3 位数字><余部>`」，**不查任何部件前缀
    /// 注册表**。上一版要求前缀必须登记在 `WeaponFamily::secondary_part()` 里，而那张表
    /// 只有三项、14 个族里 10 个为空，于是这些族的包只要带副件模型就否决整包。
    ///
    /// 拆解与两条守卫由 [`split_weapon_stem`] 提供，与磁盘改名、MRL3 引用改写**共用同一份
    /// 实现**——识别与改名必须对同一个名字得出同一个结论，否则会分叉。
    pub fn parse_for_main(value: &str, main_id: &WeaponMainId) -> Result<Self, WeaponFamilyError> {
        let Some(parsed) = split_weapon_stem(value, main_id) else {
            return Err(WeaponFamilyError::UnknownPart);
        };
        let parsed = parsed.map_err(|()| WeaponFamilyError::UnknownPart)?;

        Ok(Self {
            canonical: value.to_owned(),
            family: main_id.family(),
            role: main_id.family().role_for_prefix(parsed.prefix),
            number: main_id.number(),
            has_bs_prefix: main_id.has_bs_prefix(),
        })
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
