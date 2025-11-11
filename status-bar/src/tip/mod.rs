// CUSTOMIZATION: Customize tips shown in the status bar
// - See tip/data/ for individual tip implementations
// - Edit the TIPS HashMap in tip/data/mod.rs to add or remove tips
// - Add new tip files and register them in tip/data/mod.rs
// - Modify tip verbosity levels (short, medium, full) in each tip file

pub mod cache;
pub mod consts;
pub mod data;
pub mod utils;

use crate::LinePart;
use zellij_tile::prelude::*;

pub type TipFn = fn(&ModeInfo) -> LinePart;

pub struct TipBody {
    pub short: TipFn,
    pub medium: TipFn,
    pub full: TipFn,
}
