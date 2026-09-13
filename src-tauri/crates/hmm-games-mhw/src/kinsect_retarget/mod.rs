//! 猎虫是独立装备来源；不改变武器的 14 个 family 或其稳定身份。
mod catalog;
mod path;

pub(crate) use catalog::kinsect_targets;
pub use path::{KinsectId, KinsectPathError, KinsectResourceRoot};
