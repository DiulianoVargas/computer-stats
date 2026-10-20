//! The only module allowed to use `unsafe` (Constitution III). Every function here exposes a
//! safe API and maps every failure to a value — nothing panics.

pub mod cpu;
pub mod desktop_window;
pub mod drives;
pub mod gpu;
pub mod icons;
pub mod instance;
pub mod memory;
pub mod pawnio;
pub mod processes;
pub mod settings;
pub mod shell;
pub mod theme;
pub mod tray;
