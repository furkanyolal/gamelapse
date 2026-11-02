//! Library importers (Steam, Epic, GOG), added in a later phase.

/// A source of installed games.
pub trait Importer {
    /// Short identifier stored in `games.source` (e.g. `steam`).
    fn source(&self) -> &'static str;
}
