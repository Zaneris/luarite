# Repository Guidelines

## Project Structure & Modules
- `crates/engine_core/`: window loop (winit), renderer (wgpu), input, time, metrics, state.
- `crates/engine_scripting/`: Lua sandbox (whitelisted safe_base) and API bindings.
- `host/`: binary entrypoint (`luarite`), wires providers (metrics/input/window).
- `scripts/`: gameplay Lua (see `scripts/game.lua` for Pong demo).
- `assets/`: textures and other runtime assets (optional).

## Build, Test, and Dev
- `cargo run -p luarite`: run the engine + current script.
- `cargo build` / `cargo check`: build or type-check workspace.
- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings`: format + lint.
- `cargo test -p engine_scripting`: run unit + integration + property tests.
  - Integration tests live under `crates/engine_scripting/tests/`.

## Coding Style & Conventions
- Rust 2021; 4-space indent; ~100 col wrap; idiomatic modules (`snake_case`).
- Types/traits: `CamelCase`; functions/vars: `snake_case`; consts: `SCREAMING_SNAKE_CASE`.
- Coordinate system: Y-up for world/screen (origin at bottom, +Y up). Sizes/positions are in pixels.

## Script API (essentials)
- `engine.api_version` (u32) and `engine.get_capabilities()`.
- `engine.create_entity() -> id`, `engine.load_texture(path) -> tex`.
- `engine.set_transforms(arr)` stride=6: `id,x,y,rot,sx,sy` (v2 arrays).
- `engine.submit_sprites(arr)` stride=10: `id,tex,u0,v0,u1,v1,r,g,b,a`.
- `engine.get_input() -> snapshot` (e.g., `get_key("KeyW")`).
- `engine.window_size() -> (w,h)`; `engine.time()`.
- `engine.persist(key,val)` / `engine.restore(key)`; `engine.log(level,msg)`; `engine.get_metrics()`.

## Testing Guidelines
- Unit tests inline with modules for focused behavior.
- Integration/property tests under `crates/engine_scripting/tests/` (proptest for v2 arrays).
- Cover env loading, provider overrides, marshalling, and persistence.

## Commit & PRs
- Commits: imperative, concise subject; explain “why” in body if non-trivial.
- PRs: describe changes, link issues, include logs/screenshots if rendering/UI affected.
- Keep `fmt`/`clippy` clean; include run/test instructions if setup changes.
