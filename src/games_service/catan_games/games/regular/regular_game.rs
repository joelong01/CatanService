#![allow(dead_code)]
#![allow(unused_imports)]
#![macro_use]
use crate::games_service::catan_games::traits::game_info_trait::shuffle_vector;
use crate::games_service::harbors::harbor_enums::HarborType;
use crate::games_service::{
    buildings::{building::Building, building_enums::BuildingPosition, building_key::BuildingKey},
    catan_games::{
        game_enums::Direction,
        traits::{game_info_trait::GameInfoTrait, game_trait::CatanGame},
    },
    game,
    harbors::{harbor::Harbor, harbor_key::HarborKey},
    player::player::Player,
    roads::{road::Road, road_key::RoadKey},
    tiles::{self, tile::Tile, tile_enums::TileResource, tile_key::TileKey},
};

use crate::shared::models::GameError;
use crate::shared::{models::User, utility::get_id};

use actix_web::Resource;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::serde_as;
use std::fs::File;
use std::io::Write;
use std::{collections::HashMap, fmt};
use strum::IntoEnumIterator;

use super::game_info::{RegularGameInfo, REGULAR_GAME_INFO};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegularGame {
    pub id: String,
    pub players: Vec<Player>,
    #[serde_as(as = "Vec<(_, _)>")]
    pub tiles: HashMap<TileKey, Tile>,
    #[serde_as(as = "Vec<(_, _)>")]
    pub harbors: HashMap<HarborKey, Harbor>,
    #[serde_as(as = "Vec<(_, _)>")]
    pub roads: HashMap<RoadKey, Road>,
    #[serde_as(as = "Vec<(_, _)>")]
    pub buildings: HashMap<BuildingKey, Building>,
}

impl RegularGame {
    /// Creates a new instance of a RegularGame.
    ///
    /// The new function initializes a RegularGame with a single Player, who is designated as the creator.
    /// It sets up the game environment including the tiles, roads, and buildings according to the regular game information.
    ///
    /// # Parameters
    ///
    /// * creator: User - The user who creates and is initially the sole player in the game.
    ///
    /// # Returns
    ///
    /// A new RegularGame instance.
    pub fn new(creator: User) -> Self {
        let player = Player::new(creator);
        let game_info = &*REGULAR_GAME_INFO;
        let mut tiles = Self::setup_tiles(game_info);
        let roads = Self::setup_roads(&mut tiles);
        let buildings = Self::setup_buildings(&mut tiles);
        let harbors: HashMap<HarborKey, Harbor> = game_info
            .harbor_data()
            .into_iter()
            .map(|data| (data.key.clone(), data.clone()))
            .collect();
        Self {
            id: get_id(),
            players: vec![player],
            tiles: tiles,
            harbors: harbors,
            roads: roads,
            buildings: buildings,
        }
    }

    /// Sets up the game tiles according to the provided game information.
    ///
    /// The setup_tiles function creates a HashMap of TileKey and Tile pairs, each representing a unique tile in the game.
    /// The function uses information provided in RegularGameInfo to decide the number of rows per column, the resources for each tile,
    /// and the number associated with each tile for dice rolls.
    ///
    /// # Parameters
    ///
    /// * game_info: &RegularGameInfo - A reference to the RegularGameInfo struct containing necessary information to set up the tiles.
    ///
    /// # Returns
    ///
    /// A HashMap<TileKey, Tile> that represents the board state of the game, with each tile identified by a unique TileKey.
    fn setup_tiles(game_info: &RegularGameInfo) -> HashMap<TileKey, Tile> {
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
    /// Sets up the roads for the game board using provided tile information.
    ///
    /// The setup_roads function creates a HashMap of RoadKey and Road pairs, where each road is represented by a unique RoadKey.
    /// A road can have many keys depending on the reference tile. The function carefully manages multiple borrows of mutable tile references
    /// to avoid Rust's borrow checker errors.
    ///
    /// The function iterates over all tiles and their respective directions to create roads.
    /// If a road doesn't already exist for a direction in a tile, the function creates a new road and updates the road map of the tile and the game.
    /// The function then checks the neighbor tile in the direction of the current tile. If the neighbor tile doesn't already have a road in its
    /// respective direction, the function creates a new road and updates the road map of the neighbor tile and the game.
    ///
    /// # Parameters
    ///
    /// * tiles: &mut HashMap<TileKey, Tile> - A mutable reference to the HashMap containing all the tiles of the game.
    ///
    /// # Returns
    ///
    /// A HashMap<RoadKey, Road> that represents the roads of the game, with each road identified by a unique RoadKey.
    fn setup_roads(tiles: &mut HashMap<TileKey, Tile>) -> HashMap<RoadKey, Road> {
        let mut roads = HashMap::new();
        let tile_keys: Vec<TileKey> = tiles.keys().cloned().collect();
        for tile_key in tile_keys {
            for direction in Direction::iter() {
                let road_key = RoadKey::new(direction, tile_key.clone());
                if let Some(tile) = tiles.get_mut(&tile_key) {
                    if !tile.roads.contains_key(&direction) {
                        let road = Road::new(road_key.clone());
                        tile.roads.insert(direction, road.clone());
                        roads.insert(road_key.clone(), road.clone());
                    }
                }

                let neighbor_key = tile_key.get_neighbor_key(direction);
                let neighbor_direction = RegularGame::get_neighbor_direction(direction);
                let neighbor_road_key = RoadKey::new(neighbor_direction, neighbor_key.clone());
                if let Some(neighbor_tile) = tiles.get_mut(&neighbor_key) {
                    if !neighbor_tile.roads.contains_key(&neighbor_direction) {
                        let neighbor_road = Road::new(neighbor_road_key.clone());
                        neighbor_tile
                            .roads
                            .insert(neighbor_direction, neighbor_road.clone());
                        roads.insert(neighbor_road_key.clone(), neighbor_road.clone());
                    }
                }
            }
        }

        roads
    }

    /// Initializes the buildings on the game board.
    ///
    /// The setup_buildings function creates a HashMap of BuildingKey and Building pairs for the game.
    ///
    /// It iterates over all tiles on the board and each possible building position within a tile. For each building position, a BuildingKey is
    /// created and if a building doesn't already exist at this key, it's added to the buildings HashMap. Additionally, for each building, the function
    /// identifies its adjacent buildings and maintains these associations.
    ///
    /// # Parameters
    ///
    /// * tiles: &mut HashMap<TileKey, Tile> - A mutable reference to the HashMap containing all the tiles of the game.
    ///
    /// # Returns
    ///
    /// A HashMap<BuildingKey, Building> representing all the buildings in the game, each building being uniquely identified by a BuildingKey.
    fn setup_buildings(tiles: &mut HashMap<TileKey, Tile>) -> HashMap<BuildingKey, Building> {
        let mut buildings = HashMap::new();

        // Iterate over all the tiles
        for tile_key in tiles.keys() {
            // Iterate over all possible building positions for each tile
            for pos in BuildingPosition::iter() {
                let building_key = BuildingKey::new(pos, *tile_key);

                // Create the building if it doesn't exist yet
                if !buildings.contains_key(&building_key) {
                    let mut connected_tiles = Vec::new();
                    let mut aliases = Vec::new();
                    aliases.push(building_key);
                    // Get all adjacent buildings for this building key
                    let adjacent_building_keys = building_key.get_adjacent_building_keys(&tiles);
                    for adjacent_building_key in adjacent_building_keys {
                        if !buildings.contains_key(&adjacent_building_key) {
                            connected_tiles.push(adjacent_building_key.tile_key);
                            aliases.push(adjacent_building_key);
                        }
                    }

                    let building = Building::new(connected_tiles, aliases, 0, None);
                    buildings.insert(building_key, building);
                }
            }
        }

        buildings
    }
    /// Shuffles the resources and roll numbers of the tiles in the game.
    ///
    /// This function takes all the `Tile` objects in the game, excluding the desert,
    /// shuffles their resources and roll numbers, and then assigns the shuffled values
    /// back to the tiles. It ensures that the desert tile always has a roll number of 7.
    ///
    /// # Panics
    ///
    /// This function will panic if:
    ///
    /// - There is no tile with the `Desert` resource in the game.
    /// - There is no tile with a roll number of 7 in the game.
    /// - There are less resources or roll numbers than non-desert tiles in the game.
    ///
    /// # Example
    ///
    /// ```
    /// // Assuming `game` is a mutable reference to a `RegularGame` instance
    /// game.shuffle_tiles();
    /// ```
    fn shuffle_tiles(&mut self) {
        let mut tiles: Vec<Tile> = self.tiles.values().cloned().collect();
        let mut resources: Vec<TileResource> = tiles
            .iter()
            .filter(|tile| tile.current_resource != TileResource::Desert)
            .map(|tile| tile.current_resource.clone())
            .collect();
        let mut rolls: Vec<u32> = tiles
            .iter()
            .filter(|tile| tile.roll != 7)
            .map(|tile| tile.roll)
            .collect();

        shuffle_vector(&mut resources);
        shuffle_vector(&mut rolls);

        let desert_tile_index = tiles
            .iter()
            .position(|tile| tile.current_resource == TileResource::Desert)
            .expect("There better be a desert tile!");

        let seven_tile_index = tiles
            .iter()
            .position(|tile| tile.roll == 7)
            .expect("There needs to be a tile with a 7 roll");

        tiles[seven_tile_index].roll = tiles[desert_tile_index].roll;
        tiles[desert_tile_index].roll = 7;

        let mut resources_iter = resources.into_iter();
        let mut rolls_iter = rolls.into_iter();
        for tile in &mut tiles {
            if tile.current_resource == TileResource::Desert || tile.roll == 7 {
                continue;
            }
            tile.current_resource = resources_iter.next().expect("Ran out of resources!");
            tile.original_resource = tile.current_resource;
            tile.roll = rolls_iter.next().expect("Ran out of rolls!");
        }

        self.tiles.clear();
        for tile in tiles {
            self.tiles.insert(tile.key.clone(), tile);
        }
    }

    fn shuffle_harbors(&mut self) {
        let harbor_keys: Vec<HarborKey> = self.harbors.keys().cloned().collect();
        let mut harbor_types: Vec<HarborType> = self
            .harbors
            .values()
            .cloned()
            .map(|harbor| harbor.harbor_type.clone())
            .collect();

        shuffle_vector(&mut harbor_types);

        let mut new_harbors = HashMap::new();
        for (key, harbor_type) in harbor_keys.iter().zip(harbor_types.into_iter()) {
            new_harbors.insert(
                *key,
                Harbor {
                    key: *key,
                    harbor_type,
                },
            );
        }

        self.harbors = new_harbors;
    }
    /**
     * Validates that there are no adjacent tiles with a roll number of 6 or 8.
     * @param tiles - The array of TileProps objects to check.
     * @returns - A boolean value indicating whether the constraint is satisfied.
     */
    fn validate_no_adjacent_six_eight(&self) -> bool {
        for (tile_key, tile_data) in self.tiles.iter() {
            if tile_data.roll == 6 || tile_data.roll == 8 {
                let surrounding_tile_keys = tile_key.get_adjacent_keys();
                if surrounding_tile_keys.iter().any(|k| {
                    if let Some(surrounding_tile) = self.tiles.get(k) {
                        surrounding_tile.roll == 6 || surrounding_tile.roll == 8
                    } else {
                        false
                    }
                }) {
                    return false;
                }
            }
        }
        true
    }
}

impl<'a> CatanGame<'a> for RegularGame {
    type Players = &'a Vec<Player>;
    type Tiles = &'a mut HashMap<TileKey, Tile>;
    type GameInfoType = RegularGameInfo;

    // type Harbors = &'a HashMap<HarborKey, Harbor>;
    // type Roads = &'a HashMap<RoadKey, Road>;
    // type Buildings = &'a HashMap<BuildingKey, Building>;

    fn get_game_info(&'a self) -> &'a Self::GameInfoType {
        &*REGULAR_GAME_INFO
    }
    fn get_game_info_ro(&self) -> &Self::GameInfoType {
        &*REGULAR_GAME_INFO
    }

    fn get_tiles(&mut self) -> &mut HashMap<TileKey, Tile> {
        &mut self.tiles
    }
    fn get_tiles_ro(&self) -> &HashMap<TileKey, Tile> {
        &self.tiles
    }
    fn get_players(&self) -> &Vec<Player> {
        &self.players
    }

    fn get_neighbor(&'a mut self, tile: &Tile, direction: Direction) -> Option<Tile> {
        let current_coord = &tile.key;
        let neighbor_coord = current_coord.get_neighbor_key(direction);
        self.get_tiles().get(&neighbor_coord).cloned()
    }
    fn get_neighbor_direction(pos: Direction) -> Direction
    where
        Self: Sized,
    {
        match pos {
            Direction::North => Direction::South,
            Direction::NorthEast => Direction::SouthWest,
            Direction::SouthEast => Direction::NorthWest,
            Direction::South => Direction::North,
            Direction::SouthWest => Direction::NorthEast,
            Direction::NorthWest => Direction::SouthEast,
        }
    }

    fn debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Regular Game: {{ ")?;
        self.debug(f)?;
        write!(f, " }}")
    }

    fn add_user(&mut self, user: User) {
        self.players.push(Player::new(user));
    }

    fn shuffle(&mut self) {
        let mut count = 0;

        loop {
            count += 1;
            self.shuffle_tiles();

            if self.validate_no_adjacent_six_eight() {
                break;
            }
            if count % 10 == 0 {
                println!("looped {} times", count);
            }
        }

        println!("looped {} times", count);
        self.shuffle_harbors();
    }
    /// Sets the order of the players in the game.
    ///
    /// The input `id_order` specifies the desired order of the players by their IDs. The function will sort the `players`
    /// field of the `RegularGame` instance according to this order.
    ///
    /// # Arguments
    ///
    /// * `id_order` - A vector of strings representing the order of the player IDs. The size of the vector must match
    ///   the number of players in the game.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the players were successfully reordered.
    /// * `Err(GameError::PlayerMismatch)` if the number of IDs in `id_order` does not match the number of players.
    /// * `Err(GameError::IdNotFoundInOrder)` if a player's ID is not found in `id_order`.
    ///
    /// # Panics
    ///
    /// This function will panic if any player in the game does not have an ID.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut game = RegularGame::new(...);
    /// game.set_player_order(vec!["player1".to_string(), "player2".to_string(), "player3".to_string()]).unwrap();
    /// assert_eq!(game.players[0].user_data.id.unwrap(), "player1");
    /// assert_eq!(game.players[1].user_data.id.unwrap(), "player2");
    /// assert_eq!(game.players[2].user_data.id.unwrap(), "player3");
    ///
    fn set_player_order(&mut self, id_order: Vec<String>) -> Result<(), GameError> {
        // Check if the number of players matches the number of IDs in id_order
        if self.players.len() != id_order.len() {
            return Err(GameError::PlayerMismatch);
        }

        let mut players_with_indices: Vec<(usize, Player)> = self
            .players
            .iter()
            .map(|player| {
                let player_id = player
                    .user_data
                    .id
                    .as_ref()
                    .expect("Player is missing an ID");
                let index = id_order.iter().position(|id| *id == *player_id);
                index.map(|index| (index, player.clone()))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(GameError::IdNotFoundInOrder)?;

        // Sort players by their indices in id_order
        players_with_indices.sort_by_key(|(index, _player)| *index);

        // Remove indices, leaving only the sorted players
        let sorted_players = players_with_indices
            .into_iter()
            .map(|(_index, player)| player)
            .collect();

        self.players = sorted_players;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        games_service::{catan_games::traits::game_trait::CatanGame, tiles::tile_key::TileKey},
        shared::models::User,
    };
    use std::fs::File;
    use std::io::Write;

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
        let key = RoadKey::new(Direction::North, TileKey::new(-1, 2, 3));

        let tk_json = serde_json::to_string(&key).unwrap();
        print!("{:#?}", tk_json);
        let deserialized_key: RoadKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(key, deserialized_key);
    }

    fn test_serialization(game: &RegularGame) {
        //   let tiles = game.get_tiles();
        // println!("{:#?}", tiles);
        //   print!("{}", serde_json::to_string_pretty(&tiles).unwrap());
        let game_json = serde_json::to_string_pretty(game).unwrap();
        assert_eq!(game_json.contains("_"), false, 
        "There should be no _ in the json.  set #[serde(rename_all = \"camelCase\")] in the struct");
        let mut file = File::create("output.json").expect("Could not create file");
        write!(file, "{}", game_json).expect("Could not write to file");

        let de_game: RegularGame = serde_json::from_str(&game_json).expect("deserializing game");

        assert_eq!(*game, de_game);
    }

    fn test_desert(game: &RegularGame) {
        let mut desert_tile = game
            .tiles
            .iter()
            .find(|(_key, tile)| tile.current_resource == TileResource::Desert)
            .expect("desert must be here")
            .1;

        assert_eq!(desert_tile.roll, 7);

        desert_tile = game
            .tiles
            .iter()
            .find(|(_key, tile)| tile.roll == 7)
            .expect("desert must be here")
            .1;

        assert_eq!(desert_tile.current_resource, TileResource::Desert);
        assert_eq!(desert_tile.original_resource, TileResource::Desert)
    }

    fn test_rolls_and_resources(game: &RegularGame) {
        let mut roll_counts: HashMap<i32, i32> = HashMap::new();

        for tile in game.tiles.values() {
            *roll_counts.entry(tile.roll as i32).or_insert(0) += 1;
        }
        //

        assert_eq!(*roll_counts.get(&2).expect("2"), 1);
        assert_eq!(*roll_counts.get(&3).expect("3"), 2);
        assert_eq!(*roll_counts.get(&4).expect("4"), 2);
        assert_eq!(*roll_counts.get(&5).expect("5"), 2);
        assert_eq!(*roll_counts.get(&6).expect("6"), 2);
        assert_eq!(*roll_counts.get(&7).expect("7"), 1);
        assert_eq!(*roll_counts.get(&8).expect("8"), 2);
        assert_eq!(*roll_counts.get(&9).expect("9"), 2);
        assert_eq!(*roll_counts.get(&10).expect("10"), 2);
        assert_eq!(*roll_counts.get(&11).expect("11"), 2);
        assert_eq!(*roll_counts.get(&12).expect("12"), 1);

        let mut resource_counts: HashMap<TileResource, i32> = HashMap::new();

        for tile in game.tiles.values() {
            *resource_counts
                .entry(tile.current_resource.clone())
                .or_insert(0) += 1;
        }

        // resource_counts.iter().for_each(|(key, value)| {
        //     println!("{}:{}", key, value);

        // });
        assert_eq!(*resource_counts.get(&TileResource::Wheat).expect("4"), 4);
        assert_eq!(*resource_counts.get(&TileResource::Wood).expect("4"), 4);
        assert_eq!(*resource_counts.get(&TileResource::Brick).expect("3"), 3);
        assert_eq!(*resource_counts.get(&TileResource::Sheep).expect("4"), 4);
        assert_eq!(*resource_counts.get(&TileResource::Ore).expect("3"), 3);
        assert_eq!(*resource_counts.get(&TileResource::Desert).expect("3"), 1);
    }
    fn test_player_order(game: &mut RegularGame){
        game.set_player_order(vec!["3".to_string(), "2".to_string(), "1".to_string()]).unwrap();
        assert_eq!(game.players[0].user_data.id.as_ref().unwrap(), "3");
        assert_eq!(game.players[1].user_data.id.as_ref().unwrap(), "2");
        assert_eq!(game.players[2].user_data.id.as_ref().unwrap(), "1");
    }
    fn create_and_add_players() -> RegularGame{
        let user = User {
            id: Some("1".to_string()),
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

        //
        //  create 2 more users and add them to the game
        let user1 = User {
            id: Some("2".to_string()),
            partition_key: Some(101),
            password_hash: Some("hash1".to_string()),
            password: Some("password1".to_string()),
            email: "test1@example.com".to_string(),
            first_name: "Jane".to_string(),
            last_name: "Smith".to_string(),
            display_name: "janesmith".to_string(),
            picture_url: "https://example.com/picture1.jpg".to_string(),
            foreground_color: "#FF0000".to_string(),
            background_color: "#00FF00".to_string(),
            games_played: Some(5),
            games_won: Some(1),
        };

        let user2 = User {
            id: Some("3".to_string()),
            partition_key: Some(202),
            password_hash: Some("hash2".to_string()),
            password: Some("password2".to_string()),
            email: "test2@example.com".to_string(),
            first_name: "Mike".to_string(),
            last_name: "Johnson".to_string(),
            display_name: "mikejohnson".to_string(),
            picture_url: "https://example.com/picture2.jpg".to_string(),
            foreground_color: "#0000FF".to_string(),
            background_color: "#FFFF00".to_string(),
            games_played: Some(8),
            games_won: Some(3),
        };
        game.add_user(user1);
        game.add_user(user2);
        game
    }
    #[test]
    fn test_regular_game() {
        let mut game = create_and_add_players();
        game.shuffle();
        test_desert(&game);
        test_rolls_and_resources(&game);
        test_player_order(&mut game);
        test_serialization(&game);
    }
}
