#![allow(dead_code)]


use serde::{Deserialize, Serialize};

use crate::games_service::shared::game_enums::{GameAction, GamePhase, GameState};

/// this trait should hold the transitions from one state to the next.  it does not operate on the concrete
/// state of the Game -- that should be done in the Game itself (for seperation of concerns reasons)
pub(crate) trait StateMachineTrait {
    fn current_state(&self) -> StateData;
    fn set_current_state(&mut self, game_state: GameState) -> Vec<GameAction>;
    fn next_state(&self) -> GameState;

}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
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
    //
    //  these are the "generic" actions that should apply to all games.
    //  the concreate Game will then call this and override the answer based on game state
    pub fn actions(&self) -> Vec<GameAction> {
        let mut actions = vec![
        GameAction::Next,
        GameAction::Undo];
        match self.game_state {
            GameState::AddingPlayers => actions.push(GameAction::AddPlayer),
               
            GameState::ChoosingBoard => actions.push(GameAction::NewBoard),
               
            GameState::SettingPlayerOrder => actions.push(GameAction::SetOrder),
            GameState::AllocateResourceForward => actions.push(GameAction::Build),
            GameState::AllocateResourceReverse => actions.push(GameAction::Build),
            GameState::WaitingForRoll => actions.push( GameAction::Roll),
            GameState::MustMoveBaron => {
               actions = vec![GameAction::MoveBaron, GameAction::Undo, GameAction::Redo];
            },
            GameState::BuyingAndTrading => actions.append(
                &mut vec![GameAction::Trade,
                GameAction::Buy,
                GameAction::Build],
            ),
            GameState::Supplemental => {
                actions.append(&mut vec![GameAction::Buy, GameAction::Build]);
            }
            GameState::GameOver => todo!(),
        }
        actions
    }
}
