// Allow dead code in this module
#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use crate::shared::models::User;

use crate::games_service::{buildings::building::Building, harbor::HarborData, roads::road::Road};

use super::calculated_state::{CalculatedState, ResourceCount};
use super::player_enums::Target;

//
//  this contains all the "concrete" data the result from a players actions.  we separetely define the calculated
//  data.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Player {
    user_data: User,
    roads: Vec<Road>,
    buildings: Vec<Building>,
    harbors: Vec<HarborData>,
    targets: Vec<Target>, // from this you can derive number of times 7 is rolled, how many knights played
    resource_count: ResourceCount, // total number of resources won and/or lost
    good_rolls: i8,       // the number of rolls the resulted in resources
    bad_rolls: i8,        // the number of rolls the resulted in no resources
    state: CalculatedState,
}

impl Player {
    pub fn new(user: User) -> Self {
        Self {
            user_data: user.clone(),
            roads: vec![],
            buildings: vec![],
            harbors: vec![],
            targets: vec![],
            resource_count: ResourceCount::default(),
            good_rolls: 0,
            bad_rolls: 0,
            state: CalculatedState::default(),
        }
    }
}
