#![allow(unused_imports)]

use crate::{
    games_service::{
        catan_games::games::regular::regular_game::RegularGame, player::player::Player, shared::game_enums::{GameAction, GameState},
    },
    shared::shared_models::{ClientUser, GameError},
};

use std::{
    borrow::{Borrow, BorrowMut},
    collections::HashMap,
    fmt,
};

use crate::games_service::{
    buildings::{building::Building, building_key::BuildingKey},
    harbors::{harbor::Harbor, harbor_key::HarborKey},
    roads::{road::Road, road_key::RoadKey},
    shared::game_enums::Direction,
    tiles::{tile::Tile, tile_key::TileKey},
};
use crate::shared::service_models::PersistUser;

use super::game_info_trait::GameInfoTrait;

pub trait GameTrait<'a> {
    type Players: Borrow<Vec<Player>>;
    type Tiles: BorrowMut<HashMap<TileKey, Tile>>;
    type GameInfoType: GameInfoTrait;

    // type Harbors: Borrow<HashMap<HarborKey, Harbor>>;
    // type Roads: Borrow<HashMap<RoadKey, Road>>;
    // type Buildings: Borrow<HashMap<BuildingKey, Building>>;

    fn get_game_info(&'a self) -> &'a Self::GameInfoType;
    fn get_game_info_ro(&self) -> &Self::GameInfoType;
    fn get_tiles(&mut self) -> &mut HashMap<TileKey, Tile>;
    fn get_tiles_ro(&self) -> &HashMap<TileKey, Tile>;
    fn get_players(&self) -> &HashMap<String, Player>;
    fn get_neighbor(&'a mut self, tile: &Tile, direction: Direction) -> Option<Tile>;
    fn get_neighbor_direction(pos: Direction) -> Direction
    where
        Self: Sized;
    fn debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;

    fn add_user(&mut self, user: &ClientUser);
    fn shuffle(&mut self);
    fn set_player_order(&mut self, id_order: Vec<String>) -> Result<(), GameError>;
    fn get_next_player(&mut self) -> Player;
    fn valid_actions(&self, can_redo: bool) -> Vec<GameAction>;
    fn current_state(&self) -> GameState;
    fn get_next_state(&self) -> GameState;
    fn set_next_state(&self) -> Result<RegularGame, GameError>;

    // fn new(creator: User) -> Self
    // where
    //     Self: Sized;
    // fn setup_tiles(&'a mut self) -> Self;
    // fn build_tiles(&'a mut self) -> HashMap<TileKey, Tile> ;
    // fn setup_roads(&'a mut self);
    // fn setup_buildings(&'a mut self);
}
