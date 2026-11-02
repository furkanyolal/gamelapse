## What and why

<!-- Describe the change and the reason for it. Link the issue if there is one. -->

## Checklist

- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [ ] `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] `npm run lint`
- [ ] `npm test -- --run`
- [ ] `npm run build`
- [ ] `src/bindings.ts` is regenerated and committed if commands or types changed
- [ ] `docs/contract.md` is updated if the IPC changed
- [ ] Commits follow Conventional Commits
