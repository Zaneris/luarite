#![deny(warnings)]

use anyhow::Result;
use engine_core::state::SpriteData;
use engine_core::window::EngineWindow;
use engine_scripting::api::{EngineApi, SpriteV2, InputSnapshot};
use engine_scripting::sandbox::LuaSandbox;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tracing::{info, Level};
use std::io::BufRead;

fn main() -> Result<()> {
    // Parse simple CLI flags for record/replay
    let mut record_path: Option<String> = None;
    let mut replay_path: Option<String> = None;
    {
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--record" => record_path = args.next(),
                "--replay" => replay_path = args.next(),
                _ => {}
            }
        }
    }
    // Reduce terminal output now that we have an on-screen HUD. Default to WARN.
    tracing_subscriber::fmt()
        .with_max_level(Level::WARN)
        .init();

    info!("Luarite Engine starting...");

    // Initialize Lua sandbox and engine API
    let sandbox = Rc::new(LuaSandbox::new()?);
    let api = EngineApi::new();

    // Shared exchange between Lua callbacks and engine update
    struct ScriptExchange {
        transforms: Vec<f64>,         // v2 flat array (scratch, reused)
        transforms_dirty: bool,
        transforms_f32: Vec<f32>,     // retained legacy typed path (not used by swap path)
        transforms_f32_dirty: bool,
        typed_buf: Option<(std::rc::Rc<std::cell::RefCell<Vec<f32>>>, usize, usize)>, // (rc buf, rows, cap)
        typed_sprites: Option<(std::rc::Rc<std::cell::RefCell<Vec<engine_core::state::SpriteData>>>, usize, usize)>,
        sprites: Vec<SpriteV2>,       // parsed sprites
        textures: Vec<(u32, String)>, // queued texture loads (id, path)
        // Per-frame drain latches to avoid double-updates within the same frame
        drained_tf32_this_frame: bool,
        drained_sprites_this_frame: bool,
        clear_color: Option<[f32;4]>,
        render_mode: Option<engine_core::state::VirtualResolution>,
    }
    impl Default for ScriptExchange {
        fn default() -> Self {
            Self { transforms: Vec::with_capacity(1024), transforms_dirty: false, transforms_f32: Vec::with_capacity(1024), transforms_f32_dirty: false, typed_buf: None, typed_sprites: None, sprites: Vec::with_capacity(1024), textures: Vec::new(), drained_tf32_this_frame: false, drained_sprites_this_frame: false, clear_color: None, render_mode: None }
        }
    }
    let exchange = Arc::new(Mutex::new(ScriptExchange::default()));

    // HUD metrics shared with Lua get_metrics()
    #[derive(Clone, Copy, Default)]
    struct HudMetrics {
        cpu_frame_ms: f64,
        sprites_submitted: u32,
        ffi_calls: u32,
    }
    let hud_metrics = Arc::new(Mutex::new(HudMetrics::default()));
    let hud_lines: Arc<Mutex<std::collections::VecDeque<String>>> = Arc::new(Mutex::new(std::collections::VecDeque::with_capacity(16)));
    // Window size shared with Lua window_size()
    let window_size = Arc::new(Mutex::new((1024u32, 768u32)));

    // Create window early to access input handle for providers
    let mut window = EngineWindow::new();

    // Replay snapshot store (shared across providers and frame callbacks)
    let replay_snapshot_global: Arc<Mutex<InputSnapshot>> = Arc::new(Mutex::new(InputSnapshot::default()));

    // Last-used input snapshot (what scripts actually saw via engine.get_input)
    let last_used_input_global: Arc<Mutex<InputSnapshot>> = Arc::new(Mutex::new(InputSnapshot::default()));

    // Install engine namespace with sinks that fill the exchange
    {
        let ex1 = exchange.clone();
        let set_transforms_cb = Rc::new(move |slice: &[f64]| {
            if let Ok(mut ex) = ex1.lock() {
                ex.transforms.clear();
                ex.transforms.extend_from_slice(slice);
                ex.transforms_dirty = true;
            }
        });
        let ex2 = exchange.clone();
        let submit_sprites_cb = Rc::new(move |sprites: &[SpriteV2]| {
            if let Ok(mut ex) = ex2.lock() {
                ex.sprites.clear();
                ex.sprites.extend_from_slice(sprites);
            }
        });
        // Typed sprites path: pass engine-owned vec Rc and rows
        let ex_sb = exchange.clone();
        let submit_sprites_typed_cb: Rc<dyn Fn(std::rc::Rc<std::cell::RefCell<Vec<SpriteData>>>, usize, usize)> = Rc::new(move |rcvec, rows, cap| {
            if let Ok(mut ex) = ex_sb.lock() {
                // Capture only the first submission per frame to avoid overwriting
                if !ex.drained_sprites_this_frame && ex.typed_sprites.is_none() {
                    ex.typed_sprites = Some((rcvec.clone(), rows, cap));
                }
            }
        });
        // Typed buffer f32 path: pass engine-owned buffer Rc and row/cap counts
        let ex_tf32 = exchange.clone();
        let set_transforms_f32_cb = Rc::new(move |rcbuf: std::rc::Rc<std::cell::RefCell<Vec<f32>>>, rows: usize, cap: usize| {
            if let Ok(mut ex) = ex_tf32.lock() {
                // Capture only first per frame; later updates in same frame are ignored by host drain anyway
                if !ex.drained_tf32_this_frame && ex.typed_buf.is_none() {
                    ex.typed_buf = Some((rcbuf.clone(), rows, cap));
                }
            }
        });
        // Queue texture loads from Lua
        let ex3 = exchange.clone();
        let load_texture_cb = Rc::new(move |path: String, id: u32| {
            if let Ok(mut ex) = ex3.lock() {
                ex.textures.push((id, path));
            }
        });
        // Provider closure reads latest HUD metrics for Lua
        let hud_provider = {
            let hm = hud_metrics.clone();
            Rc::new(move || {
                if let Ok(m) = hm.lock() {
                    (m.cpu_frame_ms, m.sprites_submitted, m.ffi_calls)
                } else {
                    (0.0, 0, 0)
                }
            })
        };
        // Input providers (live vs replay)
        let input_handle = window.input_handle();
        let last_used_for_live = last_used_input_global.clone();
        let live_input_provider: Rc<dyn Fn() -> InputSnapshot> = Rc::new(move || {
            let mut snap = InputSnapshot::default();
            if let Ok(inp) = input_handle.lock() {
                snap.mouse_x = inp.mouse_x;
                snap.mouse_y = inp.mouse_y;
                for k in inp.keys.iter() { snap.keys.insert(k.clone(), true); }
                for b in inp.mouse_buttons.iter() { snap.mouse_buttons.insert(b.clone(), true); }
            }
            if let Ok(mut dst) = last_used_for_live.lock() { *dst = snap.clone(); }
            snap
        });
        // Replay snapshot to be filled each frame (end of frame) if replaying
        let replay_snapshot = replay_snapshot_global.clone();
        let last_used_for_replay = last_used_input_global.clone();
        let replay_input_provider: Rc<dyn Fn() -> InputSnapshot> = {
            let rs = replay_snapshot.clone();
            Rc::new(move || {
                if let Ok(snap) = rs.lock() {
                    if let Ok(mut dst) = last_used_for_replay.lock() { *dst = snap.clone(); }
                    return snap.clone();
                }
                let d = InputSnapshot::default();
                if let Ok(mut dst) = last_used_for_replay.lock() { *dst = d.clone(); }
                d
            })
        };
        let input_provider: Rc<dyn Fn() -> InputSnapshot> = if replay_path.is_some() { replay_input_provider } else { live_input_provider };
        api.setup_engine_namespace_with_sinks_and_metrics(
            sandbox.lua(),
            set_transforms_cb,
            Some(set_transforms_f32_cb),
            submit_sprites_cb,
            Some(submit_sprites_typed_cb),
            hud_provider,
            load_texture_cb,
            input_provider,
            {
                let ws = window_size.clone();
                Rc::new(move || {
                    if let Ok(v) = ws.lock() { *v } else { (1024,768) }
                })
            },
            {
                let h = hud_lines.clone();
                Rc::new(move |msg: String| {
                    if let Ok(mut q) = h.lock() {
                        if q.len() >= 12 { q.pop_front(); }
                        q.push_back(msg);
                    }
                })
            },
            {
                let ex_cc = exchange.clone();
                Rc::new(move |r,g,b,a| {
                    if let Ok(mut ex) = ex_cc.lock() { ex.clear_color = Some([r,g,b,a]); }
                })
            },
            {
                let ex_rm = exchange.clone();
                Rc::new(move |mode: &'static str| {
                    println!("DEBUG: set_render_resolution called with mode: {}", mode);
                    if let Ok(mut ex) = ex_rm.lock() {
                        let resolution = if mode == "retro" { 
                            println!("DEBUG: Setting Retro320x180 resolution");
                            engine_core::state::VirtualResolution::Retro320x180 
                        } else { 
                            println!("DEBUG: Setting Hd1920x1080 resolution");
                            engine_core::state::VirtualResolution::Hd1920x1080 
                        };
                        ex.render_mode = Some(resolution);
                    }
                })
            },
        )?;
    }

    // Load the main game script from disk (enables reload)
    const SCRIPT_PATH: &str = "scripts/game.lua";
    let script_src = std::fs::read_to_string(SCRIPT_PATH)?;
    sandbox.load_script(&script_src, "game.lua")?;
    let mut last_mtime = std::fs::metadata(SCRIPT_PATH).and_then(|m| m.modified()).ok();

    // Wire script lifecycle into engine window
    {
        let sandbox_for_start = sandbox.clone();
        window.set_script_on_start(move |_state| {
            // Call Lua on_start if present
            if let Err(e) = sandbox_for_start.call_function::<(), ()>("on_start", ()) {
                tracing::warn!("on_start not called: {}", e);
            }
        });

        let sandbox_for_update = sandbox.clone();
        let mut api_for_update = api; // move into closure to keep time updated
        let exchange_for_update = exchange.clone();
        let sandbox_for_reload = sandbox.clone();
        let mut quiesce_frames: u8 = 0;
        let mut reload_key_down = false;
        let mut sprites_scratch: Vec<SpriteData> = Vec::with_capacity(1024);
        // Capture only what we need (avoid capturing `window` by value)
        let window_input_for_reload = window.input_handle();
        window.set_script_on_update(move |dt, state| {
            // Advance engine time for Lua view
            api_for_update.update_time(dt);

            // Manual reload on 'R'
            if let Ok(inp) = window_input_for_reload.lock() {
                let is_down = inp.keys.contains("KeyR");
                if is_down && !reload_key_down {
                    if let Ok(src) = std::fs::read_to_string(SCRIPT_PATH) {
                        if let Err(e) = sandbox_for_reload.reload_script(&src, "game.lua") {
                            tracing::error!("Manual reload failed: {}", e);
                        } else {
                            tracing::info!("Manual script reload triggered");
                            quiesce_frames = 1;
                        }
                    }
                }
                reload_key_down = is_down;
            }

            // File watcher: reload if modified
            if let Ok(meta) = std::fs::metadata(SCRIPT_PATH) {
                if let Ok(modified) = meta.modified() {
                    if Some(modified) != last_mtime {
                        match std::fs::read_to_string(SCRIPT_PATH) {
                            Ok(src) => {
                                if let Err(e) = sandbox_for_reload.reload_script(&src, "game.lua") {
                                    tracing::error!("Reload failed: {}", e);
                                } else {
                                    tracing::info!("Script reloaded: {}", SCRIPT_PATH);
                                    last_mtime = Some(modified);
                                    quiesce_frames = 1; // skip next on_update
                                }
                            }
                            Err(e) => tracing::warn!("Failed to read script: {}", e),
                        }
                    }
                }
            }

            // Call Lua on_update(dt), unless quiescing this frame
            if quiesce_frames == 0 {
                if let Err(e) = sandbox_for_update.call_function::<(f64,), ()>("on_update", (dt,)) {
                    tracing::error!("on_update error: {}", e);
                }
            } else {
                quiesce_frames = quiesce_frames.saturating_sub(1);
            }

            // Drain exchange into engine state
            if let Ok(mut ex) = exchange_for_update.lock() {
                if let Some([r,g,b,a]) = ex.clear_color.take() {
                    state.set_clear_color(r,g,b,a);
                }
                if let Some(m) = ex.render_mode.take() {
                    println!("DEBUG: Applying virtual resolution to engine state: {:?}", m);
                    state.set_virtual_resolution(m);
                }
                // Handle queued texture loads
                if !ex.textures.is_empty() {
                    for (id, path) in ex.textures.drain(..) {
                        match std::fs::read(&path) {
                            Ok(bytes) => {
                                state.insert_texture_with_id(id, &path, bytes);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to load texture '{}': {}", path, e);
                            }
                        }
                    }
                }
                // Prefer zero-copy typed buffer swap if present
                if let Some((rcbuf, rows, cap)) = ex.typed_buf.take() {
                    if !ex.drained_tf32_this_frame {
                        let mut v = rcbuf.borrow_mut();
                        // Swap script buffer vec into engine state and truncate to active elems
                        state.swap_transform_buffer_with_len(&mut v, rows * 6);
                        // Ensure script buffer length is at least cap again (preserve contents)
                        v.resize(cap * 6, 0.0);
                        ex.drained_tf32_this_frame = true;
                    }
                } else if ex.transforms_f32_dirty {
                    if let Err(e) = state.set_transforms_from_f32_slice(&ex.transforms_f32) {
                        tracing::error!("Failed to set transforms (f32): {}", e);
                    }
                    ex.transforms_f32_dirty = false;
                } else if ex.transforms_dirty {
                    if let Err(e) = state.set_transforms_from_slice(&ex.transforms) {
                        tracing::error!("Failed to set transforms: {}", e);
                    }
                    ex.transforms_dirty = false;
                }
                // Prefer zero-copy typed sprites swap if present
                if let Some((rcvec, rows, _cap)) = ex.typed_sprites.take() {
                    if !ex.drained_sprites_this_frame {
                        // Copy path: write directly into EngineState without intermediate Vec moves
                        let v = rcvec.borrow();
                        if let Err(e) = state.set_sprites_from_slice(&v[..rows.min(v.len())]) {
                            tracing::error!("Failed to set sprites from slice: {}", e);
                        }
                        ex.drained_sprites_this_frame = true;
                    }
                } else if !ex.sprites.is_empty() {
                    // Fallback: parsed v2 sprites
                    sprites_scratch.clear();
                    sprites_scratch.reserve(ex.sprites.len());
                    for s in ex.sprites.drain(..) {
                        sprites_scratch.push(SpriteData { entity_id: s.entity_id, texture_id: s.texture_id, uv: [s.u0, s.v0, s.u1, s.v1], color: [s.r, s.g, s.b, s.a] });
                    }
                    if let Err(e) = state.append_sprites(&mut sprites_scratch) {
                        tracing::error!("Failed to submit sprites: {}", e);
                    }
                }
            }
        });
    }

    // Feed HUD metrics from the engine after each frame
    {
        let hm = hud_metrics.clone();
        let ws_upd = window_size.clone();
        let last_used_for_write = last_used_input_global.clone();
        // Replay snapshot handle for this closure
        let replay_snapshot_set = replay_snapshot_global.clone();
        // Exchange for resetting per-frame drain flags
        let exchange_reset = exchange.clone();
        // Optional record/replay handles
        let mut rec_file = match record_path.clone() {
            Some(p) => Some(std::fs::File::create(p).expect("create record file")),
            None => None,
        };
        let mut rep_lines: Option<std::io::Lines<std::io::BufReader<std::fs::File>>> = replay_path.clone().map(|p| {
            let f = std::fs::File::open(p).expect("open replay file");
            std::io::BufReader::new(f).lines()
        });

        window.set_on_end_frame(move |state, metrics| {
            if let Ok(mut m) = hm.lock() {
                m.cpu_frame_ms = metrics.current_metrics().cpu_frame_ms;
                m.ffi_calls = state.get_ffi_calls_this_frame();
                m.sprites_submitted = state.get_sprites().len() as u32;
            }
            // Update window size from engine_state
            let (w,h) = state.window_size();
            if let Ok(mut wh) = ws_upd.lock() { *wh = (w,h); }

            // Determinism: compute transform hash
            let h64 = state.compute_transform_hash();

            // Record: write simple input snapshot + hash per frame
            if let Some(f) = rec_file.as_mut() {
                use std::io::Write;
                // Snapshot exactly what scripts saw via engine.get_input()
                let mut keys: Vec<String> = Vec::new();
                let mut btns: Vec<String> = Vec::new();
                let mut mx = 0.0f64;
                let mut my = 0.0f64;
                if let Ok(snap) = last_used_for_write.lock() {
                    mx = snap.mouse_x; my = snap.mouse_y;
                    for (k,v) in snap.keys.iter() { if *v { keys.push(k.clone()); } }
                    for (b,v) in snap.mouse_buttons.iter() { if *v { btns.push(b.clone()); } }
                }
                keys.sort(); btns.sort();
                let keys_s = keys.join("|");
                let btns_s = btns.join("|");
                let _ = writeln!(f, "H {}\tK {}\tB {}\tMX {:.3}\tMY {:.3}", h64, keys_s, btns_s, mx, my);
            }
            // Replay: read next line, parse snapshot + expected hash; set snapshot for next frame; compare hash
            if let Some(lines) = rep_lines.as_mut() {
                if let Some(Ok(line)) = lines.next() {
                    let mut rs = InputSnapshot::default();
                    let mut expected: Option<u64> = None;
                    for tok in line.split('\t') {
                        let tok = tok.trim();
                        if let Some(rest) = tok.strip_prefix("H ") { expected = rest.parse::<u64>().ok(); }
                        else if let Some(rest) = tok.strip_prefix("K ") { for k in rest.split('|') { if !k.is_empty() { rs.keys.insert(k.to_string(), true); } } }
                        else if let Some(rest) = tok.strip_prefix("B ") { for b in rest.split('|') { if !b.is_empty() { rs.mouse_buttons.insert(b.to_string(), true); } } }
                        else if let Some(rest) = tok.strip_prefix("MX ") { rs.mouse_x = rest.parse::<f64>().unwrap_or(0.0); }
                        else if let Some(rest) = tok.strip_prefix("MY ") { rs.mouse_y = rest.parse::<f64>().unwrap_or(0.0); }
                    }
                    if let Ok(mut cur) = replay_snapshot_set.lock() { *cur = rs; }
                    if let Some(exp) = expected { if exp != h64 { tracing::error!("Determinism mismatch: expected={}, got={}", exp, h64); } }
                }
            }

            // Reset per-frame drain latches so next frame will drain once again
            if let Ok(mut ex) = exchange_reset.lock() {
                ex.drained_tf32_this_frame = false;
                ex.drained_sprites_this_frame = false;
            }
        });
    }

    // Provide HUD lines handle to engine window so it can render the overlay
    window.set_hud_lines_handle(hud_lines.clone());

    window.run()?;

    Ok(())
}
