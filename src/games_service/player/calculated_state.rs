#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct CalculatedState {
    #[serde(rename = "KnightsPlayed")]
    knights_played: i8,
    #[serde(rename = "LongestRoad")]
    longest_road: i8,

    total_resources: i8,
    won_resources: WonResources,
    has_longest_road: bool,
    has_largest_army: bool,
    city_count: i8,
    settlement_count: i8,
    road_count: i8,
    ship_count: i8,
    known_score: i8,
    times_targeted: i8,
    pip_count: i8,
    is_current_player: bool,
    max_no_resources_run: i8,
}

impl Default for CalculatedState {
    fn default() -> Self {
        Self {
            knights_played: 0,
            longest_road: 0,
            total_resources: 0,
            won_resources: Default::default(),
            has_longest_road: false,
            has_largest_army: false,
            city_count: 0,
            settlement_count: 0,
            road_count: 0,
            ship_count: 0,
            known_score: 0,
            times_targeted: 0,
            pip_count: 0,
            is_current_player: false,
            max_no_resources_run: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct WonResources {
    sheep: i8,
    wood: i8,
    wheat: i8,
    ore: i8,
    brick: i8,
    gold: i8,
}

impl WonResources {
    pub fn new(sheep: i8, wood: i8, wheat: i8, ore: i8, brick: i8, gold: i8) -> Self {
        Self {
            sheep,
            wood,
            wheat,
            ore,
            brick,
            gold,
        }
    }
}
impl Default for WonResources {
    fn default() -> Self {
        Self {
            sheep: 0,
            wood: 0,
            wheat: 0,
            ore: 0,
            brick: 0,
            gold: 0,
        }
    }
}
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ResourceCount {
    acquired: i32,
    lost: i32,
}

impl Default for ResourceCount {
    fn default() -> Self {
        Self {
            acquired: Default::default(),
            lost: Default::default(),
        }
    }
}

impl ResourceCount {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn acquired(&self) -> i32 {
        self.acquired
    }

    pub fn lost(&self) -> i32 {
        self.lost
    }
}

impl std::fmt::Display for ResourceCount {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Acquired:{} Lost:{}", self.acquired, self.lost)
    }
}
