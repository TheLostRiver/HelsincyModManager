use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameId(String);

impl GameId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::GameId;

    #[test]
    fn game_id_keeps_value() {
        let id = GameId::new("mhw");
        assert_eq!(id.as_str(), "mhw");
    }
}
