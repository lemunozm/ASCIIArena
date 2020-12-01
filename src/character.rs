use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterId {
    Player(char),
}

#[derive(Serialize, Deserialize, Debug, Clone, Builder)]
pub struct Character {
    id: CharacterId,
    symbol: char,
    max_live: usize,
    max_energy: usize,
    speed_base: f32,
}

impl Character {
    pub fn id(&self) -> CharacterId {
        self.id
    }

    pub fn symbol(&self) -> char {
        self.symbol
    }

    pub fn max_live(&self) -> usize {
        self.max_live
    }

    pub fn max_energy(&self) -> usize {
        self.max_energy
    }

    pub fn speed_base(&self) -> f32 {
        self.speed_base
    }
}

