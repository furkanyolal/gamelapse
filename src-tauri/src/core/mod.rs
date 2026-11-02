//! Core domain logic, independent of the Tauri command layer.

pub mod custom_covers;
pub mod db;
pub mod export;
pub mod idle;
pub mod importers;
pub mod limits;
pub mod metadata;
pub mod migrate_legacy;
pub mod scan;
pub mod settings;
pub mod tracker;
pub mod types;
