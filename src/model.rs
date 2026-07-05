use indexmap::IndexMap;
use crate::ast::DeclKind;

#[derive(Debug, Clone)]
pub struct Diagram {
    pub equipment: IndexMap<String, Equipment>,
    pub valves: IndexMap<String, Valve>,
    pub lines: IndexMap<String, Line>,
    pub instruments: IndexMap<String, Instrument>,
    pub signals: IndexMap<String, Signal>,
    pub junctions: IndexMap<String, Junction>,
    pub notes: IndexMap<String, Note>,
    pub groups: IndexMap<String, Group>,
    pub areas: IndexMap<String, Area>,
    pub order: Vec<(DeclKind, String)>,
}

impl Diagram {
    pub fn new() -> Self {
        Self {
            equipment: IndexMap::new(),
            valves: IndexMap::new(),
            lines: IndexMap::new(),
            instruments: IndexMap::new(),
            signals: IndexMap::new(),
            junctions: IndexMap::new(),
            notes: IndexMap::new(),
            groups: IndexMap::new(),
            areas: IndexMap::new(),
            order: Vec::new(),
        }
    }

    /// Check if an ID exists in any map.
    pub fn id_exists(&self, id: &str) -> bool {
        self.equipment.contains_key(id)
            || self.valves.contains_key(id)
            || self.lines.contains_key(id)
            || self.instruments.contains_key(id)
            || self.signals.contains_key(id)
            || self.junctions.contains_key(id)
            || self.notes.contains_key(id)
            || self.groups.contains_key(id)
            || self.areas.contains_key(id)
    }

    /// Get ports for an object ID (if it declares them).
    pub fn get_ports(&self, id: &str) -> Option<&[Port]> {
        if let Some(e) = self.equipment.get(id) {
            return Some(&e.ports);
        }
        if let Some(v) = self.valves.get(id) {
            return Some(&v.ports);
        }
        None
    }
}

impl Default for Diagram {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone)]
pub struct Port {
    pub name: String,
    pub side: Option<Side>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    North,
    South,
    East,
    West,
}

impl Side {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "north" => Some(Side::North),
            "south" => Some(Side::South),
            "east" => Some(Side::East),
            "west" => Some(Side::West),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjRef {
    pub id: String,
    pub port: Option<String>,
}

impl ObjRef {
    pub fn new(id: impl Into<String>, port: Option<String>) -> Self {
        Self { id: id.into(), port }
    }
}

impl std::fmt::Display for ObjRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.port {
            None => write!(f, "{}", self.id),
            Some(p) => write!(f, "{}.{}", self.id, p),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Equipment {
    pub id: String,
    pub equip_type: String,
    pub label: Option<String>,
    pub pos: Option<GridPos>,
    pub ports: Vec<Port>,
    /// Symbol size override in grid units (scaled by the grid scale).
    pub size: Option<GridPos>,
    /// Mount this equipment flush against another (heat pads, jackets,
    /// agitator drives). `X.port` picks the side; plain `X` means below.
    pub attach: Option<ObjRef>,
}

#[derive(Debug, Clone)]
pub struct Valve {
    pub id: String,
    pub valve_type: String,
    pub label: Option<String>,
    pub pos: Option<GridPos>,
    pub ports: Vec<Port>,
    pub actuator: Option<String>,
    pub fail: Option<String>,
    /// Normal operating state: `nc` (normally closed) / `no` (normally open).
    pub state: Option<String>,
    /// Set pressure / setpoint annotation, e.g. "5 barg".
    pub setpoint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub id: String,
    pub class: String,
    pub from: ObjRef,
    /// `None` makes an open-ended stub: a short run drawn outward from
    /// `from` (drains, vents, sample points).
    pub to: Option<ObjRef>,
    pub label: Option<String>,
    /// Draw a flexible-hose squiggle on the run.
    pub flexible: bool,
    /// Draw an insulation hatch band on the run.
    pub insulated: bool,
}

#[derive(Debug, Clone)]
pub struct Instrument {
    pub id: String,
    pub instr_type: String,
    pub label: Option<String>,
    pub pos: Option<GridPos>,
    pub attach: Option<ObjRef>,
    pub location: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Signal {
    pub id: String,
    pub sig_type: String,
    pub from: ObjRef,
    pub to: ObjRef,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Junction {
    pub id: String,
    pub pos: Option<GridPos>,
}

#[derive(Debug, Clone)]
pub struct Note {
    pub id: String,
    pub text: Option<String>,
    pub pos: Option<GridPos>,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub id: String,
    pub members: Vec<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Area {
    pub id: String,
    pub label: Option<String>,
    pub bounds: Option<(GridPos, GridPos)>,
}
