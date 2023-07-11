// RoadAlias struct
#[derive(Serialize, Deserialize, Eq, PartialEq)]
pub struct RoadAlias {
    tile_direction: Direction, // reletive to the current tile
    road_direction: Direction, // direction in the new tile
}

impl RoadAlias {
    pub fn new(tile: Direction, road: Direction) -> Self {
        Self {
            road_direction: road,
            tile_direction: tile,
        }
    }
}

impl fmt::Display for RoadAlias {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}|{}",
            self.tile_direction as i32, self.road_direction as i32
        )
    }
}
impl Hash for RoadAlias {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.road_direction.hash(state);
        self.tile_direction.hash(state);
    }
}

pub static ROAD_ALIASES: Lazy<HashMap<RoadAlias, RoadAlias>> = Lazy::new(|| {
    let mut map = HashMap::new();
    let pairs = vec![
        (
            RoadAlias::new(Direction::North, Direction::SouthEast),
            RoadAlias::new(Direction::NorthEast, Direction::NorthWest),
        ),
        (
            RoadAlias::new(Direction::NorthEast, Direction::NorthWest),
            RoadAlias::new(Direction::North, Direction::SouthEast),
        ),
        (
            RoadAlias::new(Direction::North, Direction::SouthWest),
            RoadAlias::new(Direction::NorthWest, Direction::NorthEast),
        ),
        (
            RoadAlias::new(Direction::NorthWest, Direction::NorthEast),
            RoadAlias::new(Direction::North, Direction::SouthWest),
        ),
        (
            RoadAlias::new(Direction::NorthEast, Direction::South),
            RoadAlias::new(Direction::SouthEast, Direction::North),
        ),
        (
            RoadAlias::new(Direction::SouthEast, Direction::North),
            RoadAlias::new(Direction::NorthEast, Direction::South),
        ),
        (
            RoadAlias::new(Direction::South, Direction::NorthWest),
            RoadAlias::new(Direction::SouthWest, Direction::SouthEast),
        ),
        (
            RoadAlias::new(Direction::SouthWest, Direction::SouthEast),
            RoadAlias::new(Direction::South, Direction::NorthWest),
        ),
        (
            RoadAlias::new(Direction::South, Direction::NorthEast),
            RoadAlias::new(Direction::SouthEast, Direction::SouthWest),
        ),
        (
            RoadAlias::new(Direction::SouthEast, Direction::SouthWest),
            RoadAlias::new(Direction::South, Direction::NorthEast),
        ),
    ];

    for (key, value) in pairs {
        map.insert(key, value);
    }
    map
});

pub static ADJACENT_EXTERNAL_ROADS: Lazy<HashMap<Direction, Vec<RoadAlias>>> = Lazy::new(|| {
    let mut m = HashMap::new();

    m.insert(
        Direction::North,
        vec![
            RoadAlias::new(Direction::North, Direction::SouthEast),
            RoadAlias::new(Direction::North, Direction::SouthWest),
        ],
    );

    m.insert(
        Direction::NorthEast,
        vec![
            RoadAlias::new(Direction::SouthEast, Direction::North),
            RoadAlias::new(Direction::NorthEast, Direction::NorthWest),
        ],
    );

    m.insert(
        Direction::SouthEast,
        vec![
            RoadAlias::new(Direction::SouthEast, Direction::North),
            RoadAlias::new(Direction::SouthEast, Direction::SouthWest),
        ],
    );

    m.insert(
        Direction::South,
        vec![
            RoadAlias::new(Direction::South, Direction::NorthEast),
            RoadAlias::new(Direction::South, Direction::NorthWest),
        ],
    );

    m.insert(
        Direction::SouthWest,
        vec![
            RoadAlias::new(Direction::SouthWest, Direction::SouthEast),
            RoadAlias::new(Direction::SouthWest, Direction::North),
            RoadAlias::new(Direction::NorthWest, Direction::South),
        ],
    );

    m.insert(
        Direction::NorthWest,
        vec![
            RoadAlias::new(Direction::NorthWest, Direction::South),
            RoadAlias::new(Direction::SouthWest, Direction::North),
            RoadAlias::new(Direction::North, Direction::SouthWest),
        ],
    );

    m
});