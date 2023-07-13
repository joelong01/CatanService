#![allow(unused_imports)]

use crate::games_service::player::player::Player;

use std::{
    borrow::{Borrow, BorrowMut},
    collections::HashMap,
    fmt,
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
    type GameInfoType: GameInfoTrait;

    // type Harbors: Borrow<HashMap<HarborKey, HarborData>>;
    // type Roads: Borrow<HashMap<RoadKey, Road>>;
    // type Buildings: Borrow<HashMap<BuildingKey, Building>>;

    fn get_game_info(&'a self) -> &'a Self::GameInfoType;
    fn get_game_info_ro(&self) -> &Self::GameInfoType;
    fn get_tiles(&mut self) -> &mut HashMap<TileKey, Tile>;
    fn get_tiles_ro(&self) -> &HashMap<TileKey, Tile>;
    fn get_players(&self) -> &Vec<Player>;
    fn get_neighbor(&'a mut self, tile: &Tile, direction: Direction) -> Option<Tile>;
    fn get_neighbor_direction(pos: Direction) -> Direction
    where
        Self: Sized;
    fn debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;

    fn add_user(&mut self, user: User);
    fn shuffle(&mut self);

    // fn new(creator: User) -> Self
    // where
    //     Self: Sized;
    // fn setup_tiles(&'a mut self) -> Self;
    // fn build_tiles(&'a mut self) -> HashMap<TileKey, Tile> ;
    // fn setup_roads(&'a mut self);
    // fn setup_buildings(&'a mut self);
}
