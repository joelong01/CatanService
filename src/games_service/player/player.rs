// Allow dead code in this module
#![allow(dead_code)]
use serde::{Deserialize, Serialize};

use crate::shared::models::ClientUser;

use crate::games_service::{
    buildings::building::Building, harbors::harbor::Harbor, roads::road::Road,
};

use super::calculated_state::{CalculatedState, ResourceCount};
use super::player_enums::Target;

//
//  this contains all the "concrete" data the result from a players actions.  we separetely define the calculated
//  data.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub user_data: ClientUser,
    pub roads: Vec<Road>,
    pub buildings: Vec<Building>,
    pub harbors: Vec<Harbor>,
    pub targets: Vec<Target>, // from this you can derive number of times 7 is rolled, how many knights played
    pub resource_count: ResourceCount, // total number of resources won and/or lost
    pub good_rolls: i8,       // the number of rolls the resulted in resources
    pub bad_rolls: i8,        // the number of rolls the resulted in no resources
    pub state: CalculatedState,
}

impl Player {}
