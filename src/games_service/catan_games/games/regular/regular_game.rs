#![allow(dead_code)]

use crate::games_service::{
    buildings::{building::Building, building_enums::BuildingPosition, building_key::BuildingKey},
    catan_games::{game_enums::Direction, traits::game_trait::CatanGame},
    harbor::{HarborData, HarborKey},
    player::player::Player,
    roads::{road::Road, road_key::RoadKey},
    tiles::{tile::Tile, tile_key::TileKey},
};
use crate::shared::{models::User, utility::get_id};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::IntoEnumIterator;

use super::game_info::{RegularGameInfo, REGULAR_GAME_INFO};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegularGame {
    pub id: String,
    pub players: Vec<Player>,
    pub tiles: HashMap<TileKey, Tile>,
    pub harbors: HashMap<HarborKey, HarborData>,
    pub roads: HashMap<RoadKey, Road>,
    pub buildings: HashMap<BuildingKey, Building>,
}
impl<'a> CatanGame<'a> for RegularGame {
    type Players = &'a Vec<Player>;
    type Tiles = &'a mut HashMap<TileKey, Tile>;
    type Harbors = &'a HashMap<HarborKey, HarborData>;
    type Roads = &'a HashMap<RoadKey, Road>;
    type Buildings = &'a HashMap<BuildingKey, Building>;
    type GameInfoType = RegularGameInfo;

    fn get_game_info(&'a self) -> &'a Self::GameInfoType {
        &*REGULAR_GAME_INFO
    }
    //
    //   NOTE: lifetime 'a is *not* here
    fn get_tiles(&mut self) -> &mut HashMap<TileKey, Tile> {
        &mut self.tiles
    }

    fn new(creator: User) -> Self {
        let player = Player::new(creator);
        Self {
            id: get_id(),
            players: vec![player],
            tiles: HashMap::new(),
            harbors: HashMap::new(),
            roads: HashMap::new(),
            buildings: HashMap::new(),
        }
    }
    fn setup_tiles(&mut self) {
        let tiles = self.build_tiles();
        *self.get_tiles() = tiles;
    }

    fn setup_roads(&mut self) {
        let tile_keys: Vec<TileKey> = self.tiles.keys().cloned().collect();
        for tile_key in tile_keys {
            for direction in Direction::iter() {
                let road_key = RoadKey::new(direction, tile_key.clone());
                self.tiles.entry(tile_key).and_modify(|tile| {
                    tile.roads.entry(direction).or_insert(Road::new(road_key));
                });

                let neighbor_key = &tile_key.get_neighbor_key(direction);
                if let Some(neighbor_tile) = self.tiles.get_mut(neighbor_key) {
                    let neighbor_direction = RegularGame::get_neighbor_direction(direction);
                    let neighbor_road_key = RoadKey::new(neighbor_direction, neighbor_tile.key);
                    neighbor_tile
                        .roads
                        .entry(neighbor_direction)
                        .or_insert(Road::new(neighbor_road_key));
                }
            }
        }
    }
    fn setup_buildings(&mut self) {
        let tile_keys: Vec<TileKey> = self.tiles.keys().cloned().collect();
        for tile_key in tile_keys {
            for position in BuildingPosition::iter() {
                let building_key = BuildingKey::new(position, tile_key);
                self.buildings
                    .entry(building_key)
                    .or_insert(Building::default(building_key));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        games_service::{catan_games::traits::game_trait::CatanGame, tiles::tile_key::TileKey},
        shared::models::User,
    };

    use super::*;
    #[test]
    fn test_tile_key_serialization() {
        let tile_key = TileKey::new(-1, 2, 3);

        let tk_json = serde_json::to_string(&tile_key).unwrap();
        print!("{:#?}", tk_json);
        let deserialized_tile_key: TileKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(tile_key, deserialized_tile_key);
    }
    #[test]
    fn test_road_key_serialization() {
        let key = RoadKey::new(Direction::North, TileKey { q: -1, r: 2, s: 3 });

        let tk_json = serde_json::to_string(&key).unwrap();
        print!("{:#?}", tk_json);
        let deserialized_key: RoadKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(key, deserialized_key);
    }
    #[test]
    fn test_regular_game() {
        let user = User {
            id: Some("123".to_string()),
            partition_key: Some(456),
            password_hash: Some("hash".to_string()),
            password: Some("password".to_string()),
            email: "test@example.com".to_string(),
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            display_name: "johndoe".to_string(),
            picture_url: "https://example.com/picture.jpg".to_string(),
            foreground_color: "#000000".to_string(),
            background_color: "#FFFFFF".to_string(),
            games_played: Some(10),
            games_won: Some(2),
        };
        let mut game = RegularGame::new(user);
        game.setup_tiles();
        let tiles = game.get_tiles();
        println!("{:#?}", tiles);
        print!("{:#?}", serde_json::to_string_pretty(&tiles));
        let game_json = serde_json::to_string_pretty(&game);
        print!("{:#?}", game_json)
    }
}
