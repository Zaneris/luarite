# Luarite — Rust + Lua 2D Engine (POC)

Luarite is a small, batched 2D engine written in Rust with a sandboxed Lua 5.4 scripting layer. It targets secure, deterministic gameplay with a narrow FFI surface, per‑frame performance budgets, and simple developer ergonomics.

## Highlights
- Rust core: `winit` (window/input), `wgpu` (render), `glam` (math), `tracing` (logs)
- Lua 5.4 scripting via `mlua` (vendored), strict sandbox (no `require`)
- Batched API: typed buffers (preferred) with builder; v2 flat arrays still supported
- Y‑up world coordinates (pixels); f32 transforms; texture‑ID batching
- On‑screen HUD overlay (FPS, p99, sprites, FFI) with `engine.hud_printf`
- Offscreen renderer for GPU‑free e2e tests

## Quick Start
- Run: `cargo run -p luarite`
  - Live reloads `scripts/game.lua` on save. Controls for Pong: `W/S`, `ArrowUp/ArrowDown`.
- Build: `cargo build`
- Record/Replay (determinism): `cargo run -p luarite -- --record out.log` then `--replay out.log`
- Lint/format: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`

The HUD shows FPS, CPU p99, sprites, and FFI calls. Terminal logs are quiet by default (WARN+).

## Writing Game Scripts
- Location: `scripts/game.lua` (default). The host loads from disk and hot‑reloads on file changes.
  - `require`/`dofile` are disabled in the sandbox; keep logic in one file (or implement your own loader on the host).
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
    - Optional builder: `local fb = engine.frame_builder(T, S)` then `fb:transform(i, id, x,y,rot,sx,sy)`, `fb:transform_px(i, id, x_px,y_px,rot,w_px,h_px)`, `fb:sprite_uv(i, id, u0,v0,u1,v1)`, `fb:sprite_tex(i, id, tex, u0,v0,u1,v1, r,g,b,a)`, `fb:sprite_named(i, id, atlas, name, r,g,b,a)`, `fb:sprite_color(i, r,g,b,a)`, and finalize with `fb:commit()`.
  - Legacy v2 arrays: still supported
    - `engine.set_transforms(arr)` stride=6: `id, x, y, rot, sx, sy`
    - `engine.submit_sprites(arr)` stride=10: `id, tex, u0, v0, u1, v1, r, g, b, a`
- Time, input, window
  - `engine.time() -> seconds` (fixed‑step time)
  - `engine.get_input() -> snapshot` (methods: `get_key(name)`, `get_mouse_button(name)`, `mouse_pos()`)
  - Common key names: `KeyW`, `KeyS`, `ArrowUp`, `ArrowDown`
  - `engine.window_size() -> (w, h)`
  - `engine.set_render_resolution(mode)` (`retro` or `hd`)
- Persistence, metrics, HUD
  - `engine.persist(key, value)` / `engine.restore(key)` (in‑process KV)
  - `engine.get_metrics() -> { cpu_frame_ms, sprites_submitted, ffi_calls }`
  - `engine.hud_printf(msg)` prints a line on the HUD (rate‑limited)
  - `engine.log(level, msg)` (`info|warn|error|debug`, rate‑limited)

### Rendering
- `engine.set_render_resolution(mode)`: Sets the virtual canvas resolution. Supports two modes:
  - `"retro"`: A 320x180 virtual canvas that is always integer-scaled to fit the window, preserving a pixel-perfect look.
  - `"hd"`: A 1920x1080 virtual canvas. It will use integer scaling if the window is close (within 5%) to a multiple of 1080p (e.g., 4K). Otherwise, it uses linear filtering for smooth scaling.

### Minimal Script Skeleton
```lua
assert(engine.api_version == 1)

local T, S, e, tex, fb

function on_start()
  e = engine.create_entity()
  tex = engine.load_texture("assets/atlas.png")
  engine.units.set_pixels_per_unit(64)
  T = engine.create_transform_buffer(1)
  S = engine.create_sprite_buffer(1)
  T:set_px(1, e, 100, 100, 0.0, 64, 64)
  fb = engine.frame_builder(T, S)
  fb:sprite_tex(1, e, tex, 0.0,0.0,1.0,1.0, 1.0,1.0,1.0,1.0)
end

function on_update(dt)
  -- move +Y each frame and submit once
  fb:transform_px(1, e, 100, 100 + 60*dt, 0.0, 64, 64)
  fb:commit()
end
```
### Atlas + Builder Example
```lua
assert(engine.api_version == 1)
engine.units.set_pixels_per_unit(64)

local e = engine.create_entity()
local atlas = engine.atlas_load("assets/atlas.png", "assets/atlas.json")
local tex = atlas and atlas:tex() or engine.load_texture("assets/atlas.png")

local T = engine.create_transform_buffer(1)
local S = engine.create_sprite_buffer(1)
local fb = engine.frame_builder(T, S)

function on_start()
  fb:transform_px(1, e, 200, 120, 0.0, 64, 64)
  if atlas then fb:sprite_named(1, e, atlas, "ball", 1,1,1,1) else fb:sprite_tex(1, e, tex, 0,0,1,1, 1,1,1,1) end
end

function on_update(dt)
  fb:transform_px(1, e, 200 + math.sin(engine.time())*50, 120, 0.0, 64, 64)
  fb:commit()
end
```

## API Reference

### TransformBuffer
- create: `engine.create_transform_buffer(cap)`
- set: `T:set(i, entity|id, x, y, rot, sx, sy)`
- set_px: `T:set_px(i, entity|id, x_px, y_px, rot, w_px, h_px)`
- info: `T:len()`, `T:cap()`, `T:resize(new_cap)`

### SpriteBuffer
- create: `engine.create_sprite_buffer(cap)`
- set: `S:set(i, entity, tex, u0, v0, u1, v1, r, g, b, a)`
- texture: `S:set_tex(i, entity, tex)`
- UVs: `S:set_uv_rect(i, u0, v0, u1, v1)`
- color: `S:set_color(i, r, g, b, a)`
- atlas: `S:set_named_uv(i, atlas, name)`
- info: `S:len()`, `S:cap()`, `S:resize(new_cap)`

### FrameBuilder
- create: `engine.frame_builder(T, S)`
- transform: `fb:transform(i, entity, x, y, rot, sx, sy)`
- transform_px: `fb:transform_px(i, entity, x_px, y_px, rot, w_px, h_px)`
- sprite (UV): `fb:sprite_uv(i, entity, u0, v0, u1, v1)`
- sprite (texture): `fb:sprite_tex(i, entity, tex, u0, v0, u1, v1, r, g, b, a)`
- sprite (atlas): `fb:sprite_named(i, entity, atlas, name, r, g, b, a)`
- sprite_color: `fb:sprite_color(i, r, g, b, a)`
- commit: `fb:commit()`

### HUD & Logging
- `engine.hud_printf("L:1 R:0 fps=60.0")` shows on‑screen; max ~12 lines kept, 30 msgs/sec rate limit.
- `engine.log("warn", "message")` emits to tracing; console defaults to WARN level.

### Performance Tips
- Prefer typed buffers + frame builder. Reuse buffers; overwrite rows in place.
- Aim for one `engine.set_transforms(...)` + one `engine.submit_sprites(...)` per frame.
- Batch by texture (use atlases). Keep SpriteBuffer small and tight to active rows.
- Transforms are f32 internally; rebuild them each frame; sprites can be updated only when attributes change.

## Project Layout
- `crates/engine_core/`: window, renderer, HUD, offscreen, input, time, metrics, state
- `crates/engine_scripting/`: Lua sandbox + API bindings (typed buffers, builder, atlas)
- `host/`: desktop runner (`luarite`) wiring (hot‑reload, record/replay, input)
- `scripts/`: Lua scripts (entry at `scripts/game.lua`)
- `assets/`: textures/atlases (falls back to white if missing)

## Testing
- Workspace: `cargo test` (unit + integration + offscreen e2e)
- Host e2e tests live in `host/tests` and catch regressions (no‑flicker, persistence, precedence).

## Notes
- POC focused on secure, deterministic scripting. Record/replay and richer metrics available; features are evolving.
