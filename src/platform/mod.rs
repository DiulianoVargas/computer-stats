//! The only module allowed to use `unsafe` (Constitution III). Every function here exposes a
//! safe API and maps every failure to a value — nothing panics.

pub mod theme;
pub mod tray;
