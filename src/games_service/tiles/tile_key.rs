#![allow(dead_code)]

use crate::games_service::catan_games::game_enums::Direction;
use once_cell::sync::Lazy;

use ::serde::{Deserialize, Serialize};

use std::collections::HashMap;
use strum::IntoEnumIterator;

// Initialize directions as a static Lazy HashMap
static DIRECTIONS: Lazy<HashMap<Direction, TileKey>> = Lazy::new(|| {
    let mut directions = HashMap::new();
    directions.insert(Direction::North, TileKey::new(0, -1, 1));
    directions.insert(Direction::NorthEast, TileKey::new(1, -1, 0));
    directions.insert(Direction::SouthEast, TileKey::new(1, 0, -1));
    directions.insert(Direction::South, TileKey::new(0, 1, -1));
    directions.insert(Direction::SouthWest, TileKey::new(-1, 1, 0));
    directions.insert(Direction::NorthWest, TileKey::new(-1, 0, 1));
    directions
});
#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TileKey {
    pub q: i32,
    pub r: i32,
    pub s: i32,
}

impl TileKey {
    // Create a new HexCoords object with the specified q, r, and s values
    pub fn new(q: i32, r: i32, s: i32) -> Self {
        Self { q, r, s }
    }

    // Get the neighboring hex coordinates at the specified position
    // usage:
    //  let tile = tiles[hex_coord].unwrap()
    //  let neighbor =
    pub fn get_neighbor_key(&self, dir: Direction) -> Self {
        let direction = DIRECTIONS.get(&dir).unwrap();
        Self::new(
            self.q + direction.q,
            self.r + direction.r,
            self.s + direction.s,
        )
    }

    //
    //  get all surrounding keys, representing all adjacent tiles - note, a lookup to the tiles hashmap
    //  must happen, as some of these tiles might be invalid, depending on the board layout
    pub fn get_adjacent_keys(&self) -> Vec<TileKey> {
        let mut tiles = vec![];
        for direction in Direction::iter() {
            tiles.push(self.get_neighbor_key(direction))
        }
        tiles
    }

    // Convert the HexCoords object to a string as we use this as a key to a HexMap and serde can't
    // serialize that, so we use a string when we serialize
    pub fn to_string(&self) -> String {
        format!("TileKey(q:{}, r:{}, s:{})", self.q, self.r, self.s)
    }

    // Create a new HexCoords object with the same q, r, and s values as this one
    pub fn clone(&self) -> Self {
        Self::new(self.q, self.r, self.s)
    }

    // Assign the values of another HexCoords object to this one

    pub fn assign(&mut self, other: &TileKey) {
        self.q = other.q;
        self.r = other.r;
        self.s = other.s;
    }

    // Check if this HexCoords object is equal to another one
    pub fn equals(&self, other: &TileKey) -> bool {
        self.q == other.q && self.r == other.r && self.s == other.s
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_tile_key_serialization() {
        let key = TileKey::new(-1, 2, 3);
        let tk_json = serde_json::to_string(&key).unwrap();
        let deserialized_key: TileKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(key, deserialized_key);
    }
}
