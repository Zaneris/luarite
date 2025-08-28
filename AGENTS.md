# Repository Guidelines

## Project Structure & Modules
- `crates/engine_core/`: rendering, window loop, input, time, metrics, state.
- `crates/engine_scripting/`: Lua sandbox (`sandbox.rs`) and engine API bindings (`api.rs`).
- `host/`: binary entrypoint (`luarite`); starts tracing, sandbox, and window.
- `scripts/`: example Lua gameplay scripts (see `game.lua`).
- `assets/`: textures and other runtime assets.
- `docs/`, `ci/`: documentation and CI scaffolding.

## Build, Test, and Dev
- `cargo build`: build the full workspace.
- `cargo run -p luarite`: run the host binary from the workspace root.
- `cargo check`: fast type-check for all crates.
- `cargo fmt --all`: format Rust code with rustfmt.
- `cargo clippy --all-targets -- -D warnings`: lint and fail on warnings.
- `cargo test`: run unit/property tests (proptest available as a dev-dep).

## Coding Style & Naming
- Rust 2021; 4-space indent; wrap at ~100 cols.
- Files/modules: `snake_case` (e.g., `engine_core/src/window.rs`).
- Types/traits: `CamelCase`; functions/vars: `snake_case`; consts: `SCREAMING_SNAKE_CASE`.
- Prefer small focused modules; keep platform code in `engine_core` and script-facing APIs in `engine_scripting`.

## Testing Guidelines
- Place unit tests inline with modules using `#[cfg(test)] mod tests { ... }`.
- Name tests after behavior, not implementation details (e.g., `resizes_surface_on_window_resize`).
- Use `proptest` for data-heavy or boundary behaviors.
- Run `cargo test` locally; add tests for new public APIs and critical paths.

## Commit & Pull Requests
- Commits: concise, imperative subject (<=72 chars), optional body explaining why/how.
  - Example: `Init renderer: add sprite pipeline + swapchain resize`.
- PRs: clear description, link issues, list changes, and include screenshots/log snippets when UI/renderer output changes.
- Required: green `fmt` and `clippy`; include run instructions if setup changed.

## Lua Scripts & Security
- Scripts live in `scripts/`; assets in `assets/`. Load via engine API (e.g., `engine.load_texture("assets/atlas.png")`).
- Sandbox blocks `io`, `os`, `require`, `dofile`, etc. Do not bypass; all I/O through `engine` API.
- Use v2 flat arrays for batching: `set_transforms` stride=6; `submit_sprites` stride=10. Reuse arrays to reduce GC (see `scripts/game.lua`).

