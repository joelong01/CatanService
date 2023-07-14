mod tests {
    use openssl::x509::verify;

    use crate::{
        games_service::{
            catan_games::{
                game_enums::{Direction, GameActions, GameState},
                games::regular::{game_state::GamePhase, regular_game::RegularGame},
                traits::game_trait::CatanGame,
            },
            roads::road_key::RoadKey,
            tiles::{tile_enums::TileResource, tile_key::TileKey},
        },
        shared::models::User,
    };
    use std::io::Write;
    use std::{collections::HashMap, fs::File};

    use super::*;

    #[test]
    fn test_regular_game() {
        println!("test_regular_game");
        let mut game = create_game(); // GameState::AddingPlayers
        test_add_players(&mut game);
        game.state_machine.next_state(); // GameState::ChoosingBoard
        test_shuffle(&mut game);
        test_shuffle(&mut game);
        game.state_machine.next_state(); // GameState::SettingPlayerOrder
        test_player_order(&mut game);

        game.state_machine.next_state(); // GameState::WaitingForStart
        verify_state_and_actions(
            &game,
            "WaitingForStart",
            GamePhase::SettingUp,
            GameState::WaitingForStart,
            vec![GameActions::Done],
        );
        game.state_machine.next_state(); // GameState::AllocateResourcesForward
        test_allocate_resources(&mut game);

        test_serialization(&game);
    }

    fn verify_state_and_actions(
        game: &RegularGame,
        name: &str,
        phase: GamePhase,
        expected_state: GameState,
        expected_actions: Vec<GameActions>,
    ) {
        println!("{}", name);
        assert_eq!(game.state_machine.phase, phase);
        assert_eq!(game.state_machine.game_state, expected_state);

        let actions = game.state_machine.actions.clone();
        assert!(
            expected_actions
                .iter()
                .all(|action| actions.contains(action)),
            "Not all expected actions are present"
        );
    }
    #[test]
    fn test_tile_key_serialization() {
        println!("test_tile_key_serialization");
        let tile_key = TileKey::new(-1, 2, 3);

        let tk_json = serde_json::to_string(&tile_key).unwrap();
        print!("{:#?}", tk_json);
        let deserialized_tile_key: TileKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(tile_key, deserialized_tile_key);
    }
    #[test]
    fn test_road_key_serialization() {
        println!("test_road_key_serialization");
        let key = RoadKey::new(Direction::North, TileKey::new(-1, 2, 3));

        let tk_json = serde_json::to_string(&key).unwrap();
        print!("{:#?}", tk_json);
        let deserialized_key: RoadKey = serde_json::from_str(&tk_json).unwrap();
        assert_eq!(key, deserialized_key);
    }

    fn test_serialization(game: &RegularGame) {
        println!("test_serialization");
        let game_json = serde_json::to_string_pretty(game).unwrap();
        assert_eq!(game_json.contains("_"), false, 
        "There should be no _ in the json.  set #[serde(rename_all = \"camelCase\")] in the struct");
        let mut file = File::create("output.json").expect("Could not create file");
        write!(file, "{}", game_json).expect("Could not write to file");

        let de_game: RegularGame = serde_json::from_str(&game_json).expect("deserializing game");

        assert_eq!(*game, de_game);
    }

    fn test_desert(game: &RegularGame) {
        println!("test_desert");
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
        println!("test_rolls_and_resources");
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
    fn test_player_order(game: &mut RegularGame) {
        println!("test_player_order");
        assert_eq!(game.state_machine.phase, GamePhase::SettingUp);
        assert_eq!(game.state_machine.game_state, GameState::SettingPlayerOrder);
        let expected_actions = vec![GameActions::Done, GameActions::SetOrder];
        let actions = game.state_machine.actions.clone();
        assert!(
            expected_actions
                .iter()
                .all(|action| actions.contains(action)),
            "Not all expected actions are present"
        );

        game.set_player_order(vec!["3".to_string(), "2".to_string(), "1".to_string()])
            .unwrap();

        assert_eq!(game.player_order[0], "3");
        assert_eq!(game.player_order[1], "2");
        assert_eq!(game.player_order[2], "1");

        assert_eq!(game.current_player_id, "3");

        let p = game.get_next_player();
        assert_eq!(game.current_player_id, p.user_data.id.unwrap());
        let p = game.get_next_player();
        assert_eq!(game.current_player_id, p.user_data.id.unwrap());
        assert_eq!(game.current_player_id, "1");

        let p = game.get_next_player();
        assert_eq!(game.current_player_id, p.user_data.id.unwrap());
        assert_eq!(game.current_player_id, "3");
    }

    fn create_game() -> RegularGame {
        println!("create_game");
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
        RegularGame::new(user)
    }
    fn test_add_players(game: &mut RegularGame) {
        let expected_actions = vec![
            GameActions::AddPlayer,
            GameActions::RemovePlayer,
            GameActions::Done,
        ];
        verify_state_and_actions(
            game,
            "test_add_players",
            GamePhase::SettingUp,
            GameState::AddingPlayers,
            expected_actions,
        );
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
    }

    fn test_shuffle(game: &mut RegularGame) {
        let expected_actions = vec![GameActions::NewBoard, GameActions::Done];
        verify_state_and_actions(
            game,
            "test_shuffle",
            GamePhase::SettingUp,
            GameState::ChoosingBoard,
            expected_actions,
        );

        game.shuffle();
        test_desert(game);
        test_rolls_and_resources(game);
    }
    fn test_allocate_resources(game: &mut RegularGame) {
        let expected_actions = vec![GameActions::Done, GameActions::Build];
        verify_state_and_actions(
            game,
            "test_allocate_resources",
            GamePhase::SettingUp,
            GameState::AllocateResourceForward,
            expected_actions,
        );
    }
}