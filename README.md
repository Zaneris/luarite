# Luarite — Rust + Lua 2D Engine (POC)

Luarite is a small, batched 2D engine written in Rust with a sandboxed Lua 5.4 scripting layer. It targets secure, deterministic gameplay with a narrow FFI surface, per‑frame performance budgets, and simple developer ergonomics.

## Highlights
- Rust core: `winit` (window/input), `wgpu` (render), `glam` (math), `tracing` (logs)
- Lua 5.4 scripting via `mlua` (vendored), whitelisted safe_base
- Batched v2 API: flat arrays for transforms/sprites (minimal crossings)
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
- Batched submission (v2 arrays)
  - `engine.set_transforms(arr)` stride=6: `id, x, y, rot, sx, sy`
  - `engine.submit_sprites(arr)` stride=10: `id, tex, u0, v0, u1, v1, r, g, b, a`
  - Tip: reuse one `transforms` and one `sprites` table; avoid creating new tables each frame
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

local transforms, sprites = {}, {}
local e, tex

function on_start()
  e = engine.create_entity()
  tex = engine.load_texture("assets/atlas.png")
  -- id,x,y,rot,sx,sy   (sx/sy in ~64px units)
  transforms[1], transforms[2], transforms[3], transforms[4], transforms[5], transforms[6] = e, 100, 100, 0.0, 1.0, 1.0
  -- id,tex,u0,v0,u1,v1,r,g,b,a
  for i=1,10 do sprites[i]=0 end
  sprites[1], sprites[2] = e, tex
  sprites[3], sprites[4], sprites[5], sprites[6] = 0.0, 0.0, 1.0, 1.0
  sprites[7], sprites[8], sprites[9], sprites[10] = 1.0, 1.0, 1.0, 1.0
end

function on_update(dt)
  transforms[2] = transforms[2] + 60*dt -- move +Y each frame
  engine.set_transforms(transforms)
  engine.submit_sprites(sprites)
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
