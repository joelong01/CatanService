
#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use crate::games_service::catan_games::game_enums::{GameActions, GameState};


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameStateMachine {
    pub game_state: GameState,
    pub phase: GamePhase,
    pub actions: Vec<GameActions>
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GamePhase {
    SettingUp,
    Playing
}
impl GameStateMachine {
    pub fn new() -> Self {
     Self {
            game_state: GameState::AddingPlayers,
            phase: GamePhase::SettingUp,
            actions: Self::get_actions(GameState::AddingPlayers)
        }
       
    }
    pub fn get_actions(state: GameState) -> Vec<GameActions> {
        match state {
            GameState::AddingPlayers => vec![
                GameActions::AddPlayer,
                GameActions::RemovePlayer,
                GameActions::Done,
            ],
            GameState::ChoosingBoard => vec![GameActions::NewBoard, GameActions::Done],
            GameState::SettingPlayerOrder => vec![GameActions::Done, GameActions::SetOrder],
            GameState::WaitingForStart => vec![GameActions::Done],
            GameState::AllocateResourceForward => vec![GameActions::Done, GameActions::Build],
            GameState::AllocateResourceReverse => vec![GameActions::Done, GameActions::Build],
            GameState::WaitingForRoll => vec![GameActions::Done, GameActions::Roll],
            GameState::MustMoveBaron => vec![GameActions::Done, GameActions::MoveBaron],
            GameState::BuyingAndTrading => vec![
                GameActions::Done,
                GameActions::Trade,
                GameActions::Buy,
                GameActions::Build,
            ],
            GameState::Supplemental => vec![
                GameActions::Done,
                GameActions::Buy,
                GameActions::Build,
            ],
        }

        
    }

    pub fn next_state(&mut self) -> GameState {
        let state =  match self.game_state {
            GameState::AddingPlayers =>  GameState::ChoosingBoard,
            GameState::ChoosingBoard => GameState::SettingPlayerOrder,
            GameState::SettingPlayerOrder => GameState::WaitingForRoll,
            GameState::WaitingForStart => GameState::AllocateResourceForward,
            GameState::AllocateResourceForward => todo!(),
            GameState::AllocateResourceReverse => todo!(),
            GameState::WaitingForRoll => todo!(),
            GameState::MustMoveBaron => todo!(),
            GameState::BuyingAndTrading => todo!(),
            GameState::Supplemental => todo!(),
        };
        self.game_state = state.clone();
        self.game_state
    }
  
}
