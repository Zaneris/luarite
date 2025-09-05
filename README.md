# Luarite 🚀

> **Rust-powered 2D game engine with elegant Lua scripting**

Luarite combines the **performance of Rust** with the **simplicity of Lua** to create beautiful 2D games. Write your game logic in expressive Lua while the engine handles rendering, input, and performance optimization under the hood.

```lua
-- Beautiful, expressive game code
engine.begin_frame()

engine.sprite{
  entity = player,
  texture = atlas,
  pos = {x, y},
  size = 32,
  color = engine.hex("#FF6B6B"),
  rotation = math.sin(engine.time()),
  z = 1
}

engine.end_frame()
```

## ✨ Why Luarite?

**🎨 Expressive** — Write games in beautiful, readable Lua with modern APIs  
**⚡ Fast** — Rust core with batched rendering and zero-allocation hot paths  
**🔒 Safe** — Sandboxed scripting with deterministic execution and replay  
**🎯 Simple** — One API, two render modes, minimal concepts to learn  
**🔧 Developer-Friendly** — Hot reload, built-in profiling, and rich debugging tools  

## 🚀 Quick Start

```bash
# Clone and run the demo
git clone https://github.com/Zaneris/luarite
cd luarite
cargo run -p luarite

# Start coding! Edit scripts/game.lua and see changes instantly
```

The demo is a fully playable Pong game. Use `W/S` and `Arrow Keys` to play!

## 🎮 Your First Game

Create a bouncing ball in just a few lines:

```lua
assert(engine.api_version == 1)

local ball = engine.create_entity()
local texture = engine.load_texture("assets/ball.png")
local x, y = 160, 90
local vx, vy = 100, 80

function on_start()
  engine.set_render_mode("retro") -- 320x180 pixel-perfect
end

function on_update(dt)
  -- Move ball
  x, y = x + vx * dt, y + vy * dt
  
  -- Bounce off walls
  if x < 16 or x > 304 then vx = -vx end
  if y < 16 or y > 164 then vy = -vy end
  
  -- Draw with beautiful API
  engine.begin_frame()
  engine.sprite{
    entity = ball,
    texture = texture,
    pos = {x, y},
    size = 32,
    color = engine.rgba(255, 255, 255, 255),
    z = 1
  }
  engine.end_frame()
end
```

## 🌈 Rich Color Support

Luarite provides multiple ways to work with colors:

```lua
-- RGB with alpha
local red = engine.rgba(255, 0, 0, 255)

-- Hex colors (CSS-style)  
local blue = engine.hex("#3498DB")
local green = engine.hex("#2ECC71FF") -- With alpha

-- HSV color space
local rainbow = engine.hsv(hue, 0.8, 0.9)

-- Raw hex integers
engine.sprite{
  entity = star,
  color = 0xFFD700FF, -- Gold
  -- ... other properties
}
```

## 🎯 Key Features

### 🖼️ Dual Rendering Modes
- **Retro Mode** (`320×180`) — Pixel-perfect integer scaling for that authentic retro feel
- **HD Mode** (`1920×1080`) — Crisp modern graphics with smart scaling

### 🎨 Modern Drawing API
- **Sugar API** — Express your vision with readable, declarative sprite calls
- **Typed Buffers** — High-performance batch rendering for advanced use cases
- **Atlas Support** — Efficient texture atlasing with named sprites
 - **Camera + Layers** — Simple camera controls with layer ordering and parallax

### 🔐 Rock-Solid Foundation
- **Deterministic** — Perfect for replays, testing, and competitive games
- **Sandboxed Lua** — Safe scripting environment with controlled access
- **Hot Reload** — Instant feedback during development

### 🛠️ Developer Experience
- **Built-in Profiler** — Real-time performance metrics in your game
- **Input Recording** — Record and replay sessions for debugging
- **Comprehensive Logging** — Structured logging with filtering

## 📖 Core Concepts

### Entities & Resources
```lua
local player = engine.create_entity()
local texture = engine.load_texture("sprites/hero.png")
local atlas = engine.atlas_load("sprites/atlas.png", "sprites/atlas.json")
```

### Input Handling
```lua
local K = engine.keys
local input = engine.get_input()

if input:down(K.KeyW) then y = y + speed * dt end
if input:pressed(K.Space) then fire_bullet() end

local mx, my = input:mouse_pos()
```

### Time & Animation
```lua
-- Smooth animations
local wobble = math.sin(engine.time() * 3) * 10

-- Deterministic random numbers
engine.seed(12345)
local random_x = engine.random_range(0, 320)
```

### Camera & Layers
Phase 1 introduces a simple camera and minimal layers that already enable parallax side‑scrollers while staying pixel‑perfect in retro mode.

```lua
-- Camera: set or move in world units
engine.camera_set({ x = 32, y = 0 })
-- engine.camera_move(dx, dy) coming soon

-- Define ordered layers (higher order draws on top)
engine.layer_define("bg",   { order = -100 })
engine.layer_define("main", { order = 0 })

-- Parallax and scroll (optional)
engine.layer_set("bg", { parallax = {0.5, 0.5} })
engine.layer_scroll("bg", 8.0 * dt, 0.0)

engine.begin_frame()
-- Background draws slower than camera due to parallax
engine.sprite{ layer = "bg",   entity = bg,   texture = tex, pos = {160, 90}, size = {320, 180}, color = engine.rgba(26,26,26,255), uv = {0,0,1,1}, z = -1 }
-- Gameplay layer (default if layer omitted)
engine.sprite{ layer = "main", entity = hero, texture = tex, pos = {hero_x, hero_y}, size = 32, color = engine.rgba(255,255,255,255), uv = {0,0,1,1}, z = 0 }
engine.end_frame()
```

Notes:
- Retro mode snaps final sprite positions to integer pixels to avoid shimmer.
- Parallax is applied by adjusting the effective camera for each layer.
- The current metrics HUD overlays the presentation surface (top‑left) and is separate from game layers; a proper UI layer over the virtual canvas will come later.

## 🏗️ Advanced Usage

### High-Performance Batching
For maximum performance, use typed buffers:

```lua
-- Pre-allocate buffers
local transforms = engine.create_transform_buffer(1000)
local sprites = engine.create_sprite_buffer(1000)
local builder = engine.frame_builder(transforms, sprites)

function on_update(dt)
  -- Batch update all entities
  for i, entity in ipairs(entities) do
    builder:transform(i, entity.id, entity.x, entity.y, 0, 32, 32)
    builder:sprite_tex(i, entity.id, texture, 0,0,1,1, 1,1,1,1)
  end
  
  builder:commit() -- Single GPU submission
end
```

### Atlas-Based Rendering
```lua
engine.sprite{
  entity = character,
  atlas = {ref = character_atlas, name = "hero_idle_01"},
  pos = {x, y},
  size = {64, 64}
}
```

## 🔧 Development Workflow

```bash
# Development
cargo run -p luarite                    # Run with hot reload

# Recording & Replay (for debugging)
cargo run -p luarite -- --record game.log
cargo run -p luarite -- --replay game.log

# Testing & Quality
cargo test                              # Run all tests
cargo fmt --all && cargo clippy --all-targets -- -D warnings
```

## 📁 Project Structure

```
luarite/
├── crates/
│   ├── engine_core/     # 🦀 Rust engine (rendering, input, windowing)
│   └── engine_scripting/ # 🌙 Lua integration and API bindings
├── host/                # 🖥️  Desktop application and hot-reload
├── scripts/             # 📜 Your Lua game scripts
└── assets/              # 🎨 Textures, atlases, and game assets
```

## 🎯 Design Philosophy

**Simplicity First** — Easy things are easy, complex things are possible  
**Performance Matters** — Zero-allocation hot paths and batched GPU operations  
**Safety & Determinism** — Predictable execution for reliable games  
**Developer Joy** — Tools and APIs that make game development delightful  

## 🤝 Contributing

We welcome contributions! Whether it's:
- 🐛 Bug fixes and performance improvements  
- ✨ New features and API enhancements
- 📚 Documentation and examples
- 🧪 Tests and quality improvements

Please see our [contribution guidelines](CONTRIBUTING.md) for details.

## 📄 License

Licensed under either of
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

---

*Built with ❤️ and modern development practices. Luarite is an experiment in AI-assisted development, combining human creativity with advanced tooling.*
