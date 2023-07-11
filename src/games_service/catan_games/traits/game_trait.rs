use crate::games_service::player::player::Player;

use std::{
    borrow::{Borrow, BorrowMut},
    collections::HashMap,
};

use crate::games_service::{
    buildings::{building::Building, building_key::BuildingKey},
    catan_games::game_enums::Direction,
    harbors::{harbor::HarborData, harbor_key::HarborKey},
    roads::{road::Road, road_key::RoadKey},
    tiles::{tile::Tile, tile_key::TileKey},
};
use crate::shared::models::User;

use super::game_info_trait::GameInfoTrait;

pub trait CatanGame<'a> {
    type Players: Borrow<Vec<Player>>;
    type Tiles: BorrowMut<HashMap<TileKey, Tile>>;
    type Harbors: Borrow<HashMap<HarborKey, HarborData>>;
    type Roads: Borrow<HashMap<RoadKey, Road>>;
    type Buildings: Borrow<HashMap<BuildingKey, Building>>;
    type GameInfoType: GameInfoTrait;

    fn get_game_info(&'a self) -> &'a Self::GameInfoType;
    fn get_tiles(&mut self) -> &mut HashMap<TileKey, Tile>;
    fn get_neighbor(&'a mut self, tile: &Tile, direction: Direction) -> Option<Tile> {
        let current_coord = &tile.key;
        let neighbor_coord = current_coord.get_neighbor_key(direction);
        self.get_tiles().get(&neighbor_coord).cloned()
    }
    fn get_neighbor_direction(pos: Direction) -> Direction {
        match pos {
            Direction::North => Direction::South,
            Direction::NorthEast => Direction::SouthWest,
            Direction::SouthEast => Direction::NorthWest,
            Direction::South => Direction::North,
            Direction::SouthWest => Direction::NorthEast,
            Direction::NorthWest => Direction::SouthEast,
        }
    }

    fn new(creator: User) -> Self;
    fn setup_tiles(&'a mut self);
    fn build_tiles(&'a mut self) -> HashMap<TileKey, Tile> {
        let game_info = self.get_game_info();
        let mut tiles = HashMap::new();
        let mid_col = game_info.rows_per_column().len() / 2;
        let mut q = -1 * ((game_info.rows_per_column())[mid_col] as i32 / 2);
        let mut r = 0;
        let mut s = -r - q;
        let mut index = 0;
        let columns = game_info.rows_per_column().len();

        for col in 0..columns {
            let rows = game_info.rows_per_column()[col];
            for _ in 0..rows {
                let tile_key = TileKey::new(q, r, s);
                let resource = game_info.tile_resources().get(index as usize).unwrap();
                let roll = game_info.rolls().get(index as usize).unwrap();
                let tile = Tile::new(tile_key, *roll, resource.clone());
                tiles.insert(tile_key, tile);
                index = index + 1;
                r = r + 1;
                s = s - 1;
            }

            q += 1;

            if col < mid_col {
                r = -(col as i32 + 1);
            } else {
                r = -(mid_col as i32)
            }

            s = -r - q;
        }
        tiles
    }
    fn setup_roads(&'a mut self);
    fn setup_buildings(&'a mut self);
}
