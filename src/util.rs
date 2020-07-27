use crate::vec2::Vec2;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Direction {
    Up, Down, Left, Right, None,
}

impl Direction {
    pub fn vec2(&self) -> Vec2 {
        match *self {
            Direction::Up => Vec2::y(-1.0),
            Direction::Down => Vec2::y(1.0),
            Direction::Right => Vec2::x(1.0),
            Direction::Left => Vec2::x(-1.0),
            Direction::None => Vec2::zero(),
        }
    }

    pub fn opposite(&self) -> Direction {
        match *self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Right => Direction::Left,
            Direction::Left => Direction::Right,
            Direction::None => Direction::None,
        }
    }
}

pub type SessionToken = usize;

pub fn format_player_names<I: IntoIterator<Item = S>, S: AsRef<str> + Ord>(players: I) -> String {
    let mut players: Vec<S> = players.into_iter().collect();
    players.sort();

    let mut formatted = String::new();
    let mut it = players.into_iter();
    if let Some(name) = it.next() {
        formatted.push_str(name.as_ref());
        for name in it {
            formatted.push_str(&format!(", {}", name.as_ref()));
        }
    }
    formatted
}

pub fn is_valid_player_name(name: &str) -> bool {
    name.len() == 1 && name.chars().all(|c| c.is_ascii_uppercase())
}

