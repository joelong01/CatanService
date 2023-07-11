#![allow(dead_code)]

use crate::games_service::catan_games::game_enums::Direction;
use once_cell::sync::Lazy;
use serde::{
    de::{Deserialize, Deserializer, Error as DeError},
    Serialize, Serializer,
};
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

#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash)]
pub struct TileKey {
    pub q: i32,
    pub r: i32,
    pub s: i32,
}

impl Serialize for TileKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for TileKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let key_str = String::deserialize(deserializer)?;
        TileKey::from_string::<D>(&key_str)
    }
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

    fn from_string<'de, D>(s: &str) -> Result<TileKey, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut map = HashMap::new();

        let tokens: Vec<_> = s
            .trim_start_matches("TileKey(")
            .trim_end_matches(')')
            .split(", ")
            .collect();

        for token in tokens {
            let mut parts = token.split(':');
            if let Some(key) = parts.next().map(str::trim).map(str::to_owned) {
                if let Some(value) = parts.next().map(str::trim).map(str::to_owned) {
                    map.insert(key, value);
                }
            }
        }

        let q = match map.get("q") {
            Some(value) => value.parse::<i32>().map_err(D::Error::custom)?,
            None => return Err(D::Error::missing_field("q")),
        };

        let r = match map.get("r") {
            Some(value) => value.parse::<i32>().map_err(D::Error::custom)?,
            None => return Err(D::Error::missing_field("r")),
        };

        let s = match map.get("s") {
            Some(value) => value.parse::<i32>().map_err(D::Error::custom)?,
            None => return Err(D::Error::missing_field("s")),
        };

        Ok(TileKey { q, r, s })
    }
}
