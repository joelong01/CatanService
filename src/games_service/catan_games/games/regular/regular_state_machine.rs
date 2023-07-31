#![allow(dead_code)]
use std::any::Any;

use crate::games_service::catan_games::games::regular::regular_game::RegularGame;
use crate::games_service::catan_games::traits::game_state_machine_trait::{
    StateData, StateMachineTrait,
};
use crate::games_service::shared::game_enums::GameState;
use crate::games_service::shared::game_models::{
    AllocateResourceForwardData, AllocateResourceReverseData,
};
use crate::shared::models::GameError;

impl StateMachineTrait for RegularGame {
    fn current_state(&self) -> StateData {
        self.state_data.clone()
    }
    fn set_current_state(&mut self, state: GameState) {
        self.state_data = StateData::new(state);
    }
    fn next_state(&mut self, action_data: Option<&dyn Any>) -> Result<GameState, GameError> {
        let state = match self.state_data.state() {
            GameState::AddingPlayers => GameState::ChoosingBoard,
            GameState::ChoosingBoard => GameState::SettingPlayerOrder,
            GameState::SettingPlayerOrder => GameState::AllocateResourceForward,
            GameState::AllocateResourceForward => {
                if let Some(data) =
                    action_data.and_then(|data| data.downcast_ref::<AllocateResourceForwardData>())
                {
                    if data.is_last {
                        GameState::AllocateResourceReverse
                    } else {
                        GameState::AllocateResourceForward
                    }
                } else if action_data.is_none() {
                    // Handle the case when action_data is None
                    GameState::AllocateResourceForward
                } else {
                    return Err(GameError::BadActionData);
                }
            }
            GameState::AllocateResourceReverse => {
                if let Some(data) =
                    action_data.and_then(|data| data.downcast_ref::<AllocateResourceReverseData>())
                {
                    if data.is_first {
                        GameState::WaitingForRoll
                    } else {
                        GameState::AllocateResourceForward
                    }
                } else if action_data.is_none() {
                    // Handle the case when action_data is None
                    GameState::AllocateResourceReverse
                } else {
                    return Err(GameError::BadActionData);
                }
            }
            GameState::WaitingForRoll => todo!(),
            GameState::MustMoveBaron => todo!(),
            GameState::BuyingAndTrading => todo!(),
            GameState::Supplemental => todo!(),
            GameState::GameOver => todo!()
        };
    
        self.state_data = StateData::new(state);
    
        Ok(state)
    }
    
}
