# Luarite — Rust + Lua 2D Engine (POC)

Luarite is a small, batched 2D engine written in Rust with a sandboxed Lua 5.4 scripting layer. It targets secure, deterministic gameplay with a narrow FFI surface, per‑frame performance budgets, and simple developer ergonomics.

## Highlights
- Rust core: `winit` (window/input), `wgpu` (render), `glam` (math), `tracing` (logs)
- Lua 5.4 scripting via `mlua` (vendored), whitelisted safe_base
- Batched API: typed buffers (zero-GC) + legacy v2 arrays
- Y‑up world coordinates (pixels), texture batching, scratch‑buffer reuse
- Tests: unit + integration + property (proptest) for marshalling and sandboxing

## Quick Start
- Run: `cargo run -p luarite`
- Build: `cargo build`
- Lint/format: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`

Demo: A simple Pong‑like script is provided in `scripts/game.lua` (controls: `W/S`, `ArrowUp/ArrowDown`). The rest of this README focuses on writing your own scripts.

## Writing Game Scripts
- Location: put your entry script at `scripts/game.lua` (default). The host embeds this file at build time.
  - To use a different file, change the path in `host/src/main.rs` (`include_str!("../../scripts/game.lua")`).
  - `require`/`dofile` are disabled in the sandbox; keep your logic in the entry script (or build your own loader in Rust).
- Lifecycle (define these globals in your script):
  - `function on_start()` — create entities, load textures, pre‑size arrays
  - `function on_update(dt)` — update positions/state and submit arrays every frame
  - Optional (planned hot‑reload): `function on_reload(old_env)`

### Coordinate System
- Y‑up: origin at bottom, +Y up; positions/scales in pixels. A scale of `1.0` maps to ~64 px.

### Engine API (essentials)
- Entities & textures
  - `engine.create_entity() -> id`
  - `engine.load_texture(path) -> tex` (host loads from disk; falls back to white if missing)
- Batched submission
  - Typed buffers (preferred):
    - `local T = engine.create_transform_buffer(cap)` with `T:set(i, id, x, y, rot, sx, sy)` or `T:set_px(i, id, x_px, y_px, rot, w_px, h_px)`; submit via `engine.set_transforms(T)`.
    - `local S = engine.create_sprite_buffer(cap)` with `S:set(i, id, tex, u0, v0, u1, v1, r, g, b, a)`, `S:set_tex`, `S:set_uv_rect`, `S:set_color`, `S:set_named_uv(i, atlas, name)`; submit via `engine.submit_sprites(S)`.
    - Optional builder: `local fb = engine.frame_builder(T, S)` then `fb:transform_px(...)`, `fb:sprite_tex(...)` or `fb:sprite_named(...)`, `fb:commit()`.
  - Legacy v2 arrays: still supported for portability
    - `engine.set_transforms(arr)` stride=6: `id, x, y, rot, sx, sy`
    - `engine.submit_sprites(arr)` stride=10: `id, tex, u0, v0, u1, v1, r, g, b, a`
- Time, input, window
  - `engine.time() -> seconds` (fixed‑step time)
  - `engine.get_input() -> snapshot` (methods: `get_key(name)`, `get_mouse_button(name)`, `mouse_pos()`)
  - Common key names: `KeyW`, `KeyS`, `ArrowUp`, `ArrowDown`
  - `engine.window_size() -> (w, h)`
- Persistence & metrics
  - `engine.persist(key, value)` / `engine.restore(key)` (in‑process KV)
  - `engine.get_metrics() -> { cpu_frame_ms, sprites_submitted, ffi_calls }`
  - `engine.log(level, msg)` where level is `"info"|"warn"|"error"|"debug"`

### Minimal Script Skeleton
```lua
assert(engine.api_version == 1)

local T, S, e, tex

function on_start()
  e = engine.create_entity()
  tex = engine.load_texture("assets/atlas.png")
  engine.units.set_pixels_per_unit(64)
  T = engine.create_transform_buffer(1)
  S = engine.create_sprite_buffer(1)
  T:set_px(1, e, 100, 100, 0.0, 64, 64)
  local fb = engine.frame_builder(T, S)
  fb:sprite_tex(1, e, tex, 0.0,0.0,1.0,1.0, 1.0,1.0,1.0,1.0)
end

function on_update(dt)
  -- move +Y each frame and submit once
  T:set_px(1, e, 100, 100 + 60*dt, 0.0, 64, 64)
  engine.set_transforms(T)
  engine.submit_sprites(S)
end
```

### Performance Tips
- Reuse `transforms` and `sprites` tables; overwrite in place.
- Use flat v2 arrays and keep to one `set_transforms` + one `submit_sprites` per frame.
- Batch by texture: prefer atlases/texture arrays; group sprites sharing the same texture.

## Project Layout
- `crates/engine_core/`: window, renderer, input, time, metrics, state
- `crates/engine_scripting/`: Lua sandbox + API bindings
- `host/`: entrypoint binary (`luarite`) wiring providers (metrics/input/window)
- `scripts/`: gameplay Lua scripts (entry at `scripts/game.lua` by default)
- `assets/`: optional textures (falls back to white if missing)

## Testing
- Scripting crate: `cargo test -p engine_scripting`
  - Unit tests (inline), integration/property tests under `crates/engine_scripting/tests/`

## Notes
- This is a POC aiming for a secure, deterministic scripting model. Hot‑reload, determinism (record/replay), richer metrics, and packaging are planned next.
