# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

- **Run**: `cargo run -p luarite` - launches the desktop runner with hot-reload of `scripts/game.lua`
- **Build**: `cargo build` - compiles all workspace crates
- **Test**: `cargo test` - runs unit, integration, and offscreen e2e tests
- **Lint/Format**: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
- **Record/Replay**: `cargo run -p luarite -- --record out.log` or `--replay out.log` for determinism testing

## Architecture Overview

Luarite is a Rust-based 2D engine with sandboxed Lua scripting. The architecture follows a clean separation:

### Core Components
- **engine_core**: Core rendering and windowing system
  - `renderer.rs`: wgpu-based batched 2D renderer with texture batching
  - `window.rs`: winit-based window management and event loop
  - `state.rs`: EngineState managing transforms and sprites 
  - `offscreen.rs`: GPU-free renderer for headless testing
  - `hud.rs`: On-screen performance overlay (FPS, p99, sprites, FFI calls)

- **engine_scripting**: Lua 5.4 sandbox and API bindings
  - `sandbox.rs`: Strict Lua environment (no require/dofile)
  - `api.rs`: Engine API bindings with typed buffers and frame builder
  - `persist.rs`: In-process key-value persistence

- **host**: Desktop runner that wires everything together
  - Hot-reloads `scripts/game.lua` on file changes
  - Zero-copy transform swapping, sprite copying to EngineState
  - Record/replay system for deterministic testing

### Key Design Patterns

**Batched API with Typed Buffers (Preferred)**:
- TransformBuffer and SpriteBuffer for efficient data management
- FrameBuilder provides ergonomic API over buffers
- Single `engine.set_transforms()` + `engine.submit_sprites()` call per frame

**Zero-Copy Data Flow**:
- Lua typed buffers → host drain (swap transforms, copy sprites) → EngineState → renderer
- Avoids per-frame allocations in hot paths

**Sandboxed Scripting**:
- Lua scripts live in `scripts/game.lua` with lifecycle functions:
  - `on_start()`: initialization
  - `on_update(dt)`: per-frame updates
- No file system access, strict FFI surface

### Virtual Canvas System
- **Retro Mode**: 320x180 virtual canvas for pixel-perfect retro games
- **HD Mode**: 1920x1080 virtual canvas for modern high-resolution games
- Coordinates work directly in virtual canvas units (not pixels)
- Engine handles scaling/letterboxing to actual window size
- Y-up coordinate system with origin at bottom-left

## Testing Strategy

- **Offscreen Tests**: GPU-free e2e tests using pixel sampling with thresholds
- **Host E2E Tests**: Cover typed drains, persistence, mixed-path precedence, flicker guards
- **Unit Tests**: Standard Rust test harness across all crates
- Tests should be named for behavior (e.g., `flicker_guard`, `persistence_static_sprites`)

## Code Style

- Rust 2021 edition, `#![deny(warnings)]`
- PascalCase types, snake_case functions/fields (e.g., `EngineState`, `set_transforms_from_f32_slice`)
- Prefer typed buffers/builders over ad-hoc data structures
- Rate-limited logging via tracing, avoid per-frame spam
- Keep APIs minimal and explicit

## Security Considerations

- Maintain strict Lua sandbox - no file/system access widening
- Avoid blocking I/O in frame loop
- No network dependencies in tests (feature-gate if needed)
- Data validation at FFI boundaries