//! The only module allowed to use `unsafe` (Constitution III). Every function here exposes a
//! safe API and maps every failure to a value — nothing panics.

pub mod cpu;
pub mod drives;
pub mod gpu;
pub mod instance;
pub mod memory;
pub mod pawnio;
pub mod shell;
pub mod theme;
pub mod tray;
