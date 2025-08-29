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
 - CLI flags: `--record path.txt` and `--replay path.txt` (determinism harness).

## Coding Style & Conventions
- Rust 2021; 4-space indent; ~100 col wrap; idiomatic modules (`snake_case`).
- Types/traits: `CamelCase`; functions/vars: `snake_case`; consts: `SCREAMING_SNAKE_CASE`.
- Coordinate system: Y-up for world/screen (origin at bottom, +Y up). Sizes/positions are in pixels.

## Script API (essentials)
- Version/caps: `engine.api_version`, `engine.get_capabilities()`
- Entities/textures: `engine.create_entity() -> id`, `engine.load_texture(path) -> tex`
- Batched submission (preferred typed buffers):
  - Transforms: `engine.create_transform_buffer(cap)`; `T:set(i,id,x,y,rot,sx,sy)`; `T:set_px(i,id,x_px,y_px,rot,w_px,h_px)`; submit via `engine.set_transforms(T)`
  - Sprites: `engine.create_sprite_buffer(cap)`; `S:set(i,id,tex,u0,v0,u1,v1,r,g,b,a)`; `S:set_tex`, `S:set_uv_rect`, `S:set_color`, `S:set_named_uv(i, atlas, name)`; submit via `engine.submit_sprites(S)`
  - Builder: `engine.frame_builder(T,S)`; `fb:transform`, `fb:transform_px`, `fb:sprite_uv`, `fb:sprite_tex`, `fb:sprite_named`, `fb:sprite_color`, `fb:commit()`
- Legacy v2 arrays (supported): `engine.set_transforms(arr stride=6)`, `engine.submit_sprites(arr stride=10)`
- Atlas: `engine.atlas_load(png_path, json_path) -> Atlas|nil`; `atlas:tex()`, `atlas:uv(name)`
- Input/time/window: `engine.get_input() -> snapshot`, `engine.time()`, `engine.window_size()`
- Units: `engine.units.set_pixels_per_unit(n)` (author in px; engine maps to units)
- Persistence/metrics/log/HUD: `engine.persist/restore`, `engine.get_metrics()`, `engine.log(level,msg)`, `engine.hud_printf(msg)`

## Testing Guidelines
- Unit tests inline with modules for focused behavior.
- Integration/property tests under `crates/engine_scripting/tests/` (proptest for v2 arrays).
- Cover env loading, provider overrides, marshalling, and persistence.

### Current test coverage map
- Sinks wiring: `integration_api.rs` covers set_transforms/submit_sprites callbacks, metrics/provider roundtrips, input provider, load_texture callback
- Sandbox: `integration_sandbox.rs` covers env usage, require blocking, missing function errors
- Property tests: `property_v2.rs` validates stride handling and sink wiring for v2 arrays
- Typed buffers: `typed_buffers.rs` covers TransformBuffer/SpriteBuffer sinks, set_px conversion, atlas_load JSON parsing
- Builder: `frame_builder.rs` covers transform/sprite commit and HUD callback

## Commit & PRs
- Commits: imperative, concise subject; explain “why” in body if non-trivial.
- PRs: describe changes, link issues, include logs/screenshots if rendering/UI affected.
- Keep `fmt`/`clippy` clean; include run/test instructions if setup changes.

## Hot-Reload & Determinism
- Hot reload: file watcher + manual `R` key; after reload, `on_start` then `on_reload(old_env)` runs; one-frame quiesce
- Determinism: record writes input snapshot (as seen by Lua) + transform buffer hash per frame; replay feeds snapshots and compares hash

## Sandbox & Security
- Whitelisted safe base; `require`/`package` locked; script environment isolated across reload (fresh env)
- `math.random` can be overridden via `engine.random` by scripts; deterministic RNG provided via `engine.seed/engine.random`

## Notes for Contributors (agent-facing)
- Keep the FFI surface narrow: one batched call per buffer per frame; avoid chatty per-entity crossings
- Zero steady-state allocations: reuse buffers, scratch vectors, and registry keys; error paths may allocate
- Pixels-first ergonomics: scripts think in pixels; respect `pixels_per_unit` for transforms
- DRY helpers exist for parsing v2 tables (`parse_transforms_table_to_out`, `parse_sprites_table_to_out`); prefer them to avoid divergence
- Base API intentionally keeps `submit_sprites(table)` stride-only validation to satisfy property tests; typed path performs stricter checks
