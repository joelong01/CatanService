#![allow(dead_code)]

use crate::{
    games_service::{
        buildings::building_enums::BuildingPosition, shared::game_enums::Direction,
    },
    shared::models::ClientUser,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use super::{road_key::RoadKey, road_enums::RoadState};

// RoadProps struct
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Road {
    primary_key: RoadKey,      // ids[0]
    aliases: Vec<RoadKey>,     // all the various ways to describe this road
    adjacent_roads: Vec<Road>, // the roads that are connected to this road
    owner: Option<ClientUser>,       // who owns this road?
    state: RoadState
}
impl Road {
    pub fn new(primary_key: RoadKey) -> Self {
        Self {
            primary_key,
            aliases: vec![],
            adjacent_roads: vec![],
            owner: None,
            state: RoadState::Unbuilt
        }
    }
}

pub static ADJACENT_INTERNAL_ROADS: Lazy<HashMap<Direction, Vec<Direction>>> = Lazy::new(|| {
    let mut map = HashMap::new();

    map.insert(
        Direction::North,
        vec![Direction::NorthEast, Direction::NorthWest],
    );
    map.insert(
        Direction::NorthEast,
        vec![Direction::SouthEast, Direction::North],
    );
    map.insert(
        Direction::SouthEast,
        vec![Direction::South, Direction::NorthEast],
    );
    map.insert(
        Direction::South,
        vec![Direction::SouthEast, Direction::SouthWest],
    );
    map.insert(
        Direction::SouthWest,
        vec![Direction::NorthWest, Direction::South],
    );
    map.insert(
        Direction::NorthWest,
        vec![Direction::North, Direction::SouthWest],
    );

    map
});

pub static DIRECTION_TO_BUILDING_POSITION_MAP: Lazy<
    HashMap<(Direction, Direction), BuildingPosition>,
> = Lazy::new(|| {
    let mut map = HashMap::new();

    map.insert(
        (Direction::North, Direction::NorthEast),
        BuildingPosition::TopRight,
    );
    map.insert(
        (Direction::NorthEast, Direction::SouthEast),
        BuildingPosition::Right,
    );
    map.insert(
        (Direction::SouthEast, Direction::South),
        BuildingPosition::BottomRight,
    );
    map.insert(
        (Direction::South, Direction::SouthWest),
        BuildingPosition::BottomLeft,
    );
    map.insert(
        (Direction::SouthWest, Direction::NorthWest),
        BuildingPosition::Left,
    );
    map.insert(
        (Direction::NorthWest, Direction::North),
        BuildingPosition::TopLeft,
    );

    // Add mappings for reversed orders
    map.insert(
        (Direction::NorthEast, Direction::North),
        BuildingPosition::TopRight,
    );
    map.insert(
        (Direction::SouthEast, Direction::NorthEast),
        BuildingPosition::Right,
    );
    map.insert(
        (Direction::South, Direction::SouthEast),
        BuildingPosition::BottomRight,
    );
    map.insert(
        (Direction::SouthWest, Direction::South),
        BuildingPosition::BottomLeft,
    );
    map.insert(
        (Direction::NorthWest, Direction::SouthWest),
        BuildingPosition::Left,
    );
    map.insert(
        (Direction::North, Direction::NorthWest),
        BuildingPosition::TopLeft,
    );

    map
});
