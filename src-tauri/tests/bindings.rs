//! Regenerates `src/bindings.ts`. Run `cargo test` after changing commands or exported types.

#[test]
fn export_typescript_bindings() {
    gamelapse_lib::export_bindings(&gamelapse_lib::specta_builder());
}
