//! Regenerates `src/bindings.ts`. Run `cargo test` after changing commands or exported types.

#[test]
fn export_typescript_bindings() {
    gametimetracker_lib::export_bindings(&gametimetracker_lib::specta_builder());
}
