#![allow(dead_code)]

use std::any::Any;

use serde::{Deserialize, Serialize};

use crate::{
    games_service::shared::game_enums::{GameAction, GamePhase, GameState},
    shared::models::GameError,
};
/// this trait should hold the transitions from one state to the next.  it does not operate on the concrete
/// state of the Game -- that should be done in the Game itself (for seperation of concerns reasons)
pub trait StateMachineTrait {
    fn current_state(&self) -> StateData;
    fn set_current_state(&mut self, game_state: GameState);
    fn next_state(&mut self, action_data: Option<&dyn Any>) -> Result<GameState, GameError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StateData {
    game_state: GameState,
}

impl StateData {
    pub fn new(state: GameState) -> Self {
        Self { game_state: state }
    }

    pub fn state(&self) -> GameState {
        self.game_state
    }

    pub fn phase(&self) -> GamePhase {
        let phase = match self.game_state {
            GameState::AddingPlayers => GamePhase::SettingUp,
            GameState::ChoosingBoard => GamePhase::SettingUp,
            GameState::SettingPlayerOrder => GamePhase::SettingUp,
            GameState::AllocateResourceForward => GamePhase::SettingUp,
            GameState::AllocateResourceReverse => GamePhase::SettingUp,
            _ => GamePhase::Playing,
        };
        phase
    }

    pub fn actions(&self) -> Vec<GameAction> {
        match self.game_state {
            GameState::AddingPlayers => vec![
                GameAction::AddPlayer,
                GameAction::RemovePlayer,
                GameAction::Done,
            ],
            GameState::ChoosingBoard => vec![GameAction::NewBoard, GameAction::Done],
            GameState::SettingPlayerOrder => vec![GameAction::Done, GameAction::SetOrder],
            GameState::AllocateResourceForward => vec![GameAction::Done, GameAction::Build],
            GameState::AllocateResourceReverse => vec![GameAction::Done, GameAction::Build],
            GameState::WaitingForRoll => vec![GameAction::Done, GameAction::Roll],
            GameState::MustMoveBaron => vec![GameAction::Done, GameAction::MoveBaron],
            GameState::BuyingAndTrading => vec![
                GameAction::Done,
                GameAction::Trade,
                GameAction::Buy,
                GameAction::Build,
            ],
            GameState::Supplemental => {
                vec![GameAction::Done, GameAction::Buy, GameAction::Build]
            }
            GameState::GameOver => todo!(),
        }
    }
}
