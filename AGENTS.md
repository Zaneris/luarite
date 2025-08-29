# Repository Guidelines

## Project Structure & Module Organization

- crates/engine_core: renderer, window/event loop, timing/metrics, and EngineState (transforms, sprites, textures). Also includes the offscreen renderer for GPU‑free tests.
- crates/engine_scripting: Lua API (mlua), sandboxing, typed buffers (TransformBuffer, SpriteBuffer), and frame builder.
- host: desktop runner; wires Lua sinks to EngineState (zero‑copy where safe), hot‑reload, record/replay.
- assets/: textures/atlases. scripts/: Lua gameplay (e.g., Pong). Tests in crates/*/tests and host/tests.

## Build, Test, and Development Commands

- Build: `cargo build` — compiles all workspace crates.
- Test: `cargo test` — runs unit/integration and offscreen render tests.
- Run host: `cargo run -p luarite` — launches the desktop runner.
- Record/Replay: `cargo run -p luarite -- --record out.log` or `--replay out.log`.

## Coding Style & Naming Conventions

- Rust 2021. Types in PascalCase; functions/fields/variables in snake_case (e.g., EngineState, SpriteData, set_transforms_from_f32_slice).
- Prefer typed buffers/builders over ad‑hoc tables. Keep APIs minimal and explicit.
- Logging via tracing; concise messages, avoid per‑frame spam. Keep HUD/log rate‑limited.

## Testing Guidelines

- Use Rust’s built‑in test harness. Place e2e/offscreen tests with the owning crate.
- Offscreen assertions sample a few pixels with thresholds (e.g., r>200, g<40, b>200) to avoid GPU variance.
- Host e2e tests cover typed drains, mixed‑path precedence, persistence across frames, and no‑flicker cadences.
- Name tests for behavior (e.g., flicker_guard, persistence_static_sprites, mixed_path_precedence).

## Commit & Pull Request Guidelines

- Commits: imperative, present tense, concise (e.g., "Fix typed sprite drain"). Group related changes.
- PRs: include summary, rationale, test coverage, repro steps, and linked issues. Add screenshots/recordings for visual changes.

## Security & Configuration Tips

- Keep the Lua sandbox strict; do not widen file/system access. Avoid blocking I/O in the frame loop.
- Avoid introducing network dependencies in tests; feature‑gate if necessary.
- Data flow: Lua → typed buffers → host drain (swap for transforms; copy for sprites) → EngineState → renderer (window/offscreen).

