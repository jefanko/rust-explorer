//! Windows Native Integration Crate
//! Safe adapters over Windows API bindings, COM, Shell, handles, and path primitives.

pub mod clipboard;
pub mod com;
pub mod enumerate;
pub mod handles;
pub mod identity;
pub mod instance;
pub mod known_folders;
pub mod path;
pub mod shell;
pub mod sink;
