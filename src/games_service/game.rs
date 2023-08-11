#![allow(dead_code)]
use std::{collections::HashMap, fmt};

use super::{
    catan_games::{
        games::regular::{game_info::RegularGameInfo, regular_game::RegularGame},
        traits::game_trait::GameTrait,
    },
    player::player::Player,
    shared::game_enums::CatanGames,
    tiles::{tile::Tile, tile_key::TileKey},
};
use crate::shared::models::ClientUser;

pub struct GameHolder<'a> {
    pub catan_game: Box<
        dyn GameTrait<
            'a,
            Players = &'a Vec<Player>,
            Tiles = &'a mut HashMap<TileKey, Tile>,
            GameInfoType = RegularGameInfo,
        >,
    >,
}
impl<'a> fmt::Debug for GameHolder<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GameHolder {{ catan_game: ")?;
        self.catan_game.debug(f)?;
        write!(f, " }}")
    }
}
impl<'a> Eq for GameHolder<'a> {}

impl<'a> PartialEq for GameHolder<'a> {
    fn eq(&self, other: &Self) -> bool {
        if self.catan_game.get_players() != other.catan_game.get_players() {
            return false;
        }
        if self.catan_game.get_tiles_ro() != other.catan_game.get_tiles_ro() {
            return false;
        }
        if self.catan_game.get_game_info_ro() != other.catan_game.get_game_info_ro() {
            return false;
        }

        true
    }
}
// impl<'a> Serialize for GameHolder<'a> {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         let json = self
//             .catan_game
//             .to_json()
//             .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
//         let mut state = serializer.serialize_struct("GameHolder", 1)?;
//         state.serialize_field("catan_game", &json)?;
//         state.end()
//     }
// }

impl<'a> GameHolder<'a> {
    pub fn new(game_type: CatanGames, creator: ClientUser) -> Self {
        let game: Box<
            dyn GameTrait<
                Players = &'a Vec<Player>,
                Tiles = &'a mut HashMap<TileKey, Tile>,
                GameInfoType = RegularGameInfo,
            >,
        > = match game_type {
            CatanGames::Regular => Box::new(RegularGame::new(&creator)),
            CatanGames::Expansion => todo!(),
            CatanGames::Seafarers => todo!(),
            CatanGames::Seafarers4Player => todo!(),
        };
        Self { catan_game: game }
    }
}

// #[cfg(test)]
// mod tests {

//     use super::*;

//     #[test]
//     fn game_marshal() {
//         let user = User {
//             id: Some("123".to_string()),
//             partition_key: Some(456),
//             password_hash: Some("hash".to_string()),
//             password: Some("password".to_string()),
//             email: "test@example.com".to_string(),
//             first_name: "John".to_string(),
//             last_name: "Doe".to_string(),
//             display_name: "johndoe".to_string(),
//             picture_url: "https://example.com/picture.jpg".to_string(),
//             foreground_color: "#000000".to_string(),
//             background_color: "#FFFFFF".to_string(),
//             games_played: Some(10),
//             games_won: Some(2),
//         };

//         let game = GameHolder::new(CatanGames::Regular, user);

//         // Serialize the game
//         let game_json = serde_json::to_string(&game).expect("Failed to serialize game");
//         print!("{}", game_json);
//         // Deserialize the game
//         // let deserialized_game: GameHolder =
//         //     serde_json::from_str(&game_json).expect("Failed to deserialize game");

//         // Assert that the deserialized game is equal to the original game
//         // assert_eq!(game, deserialized_game);
//     }
// }
