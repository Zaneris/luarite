use anyhow::Result;
use engine_core::state::SpriteData;
use engine_core::window::EngineWindow;
use engine_scripting::api::{EngineApi, SpriteV2};
use engine_scripting::sandbox::LuaSandbox;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tracing::{info, Level};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    info!("Luarite Engine starting...");

    // Initialize Lua sandbox and engine API
    let sandbox = Rc::new(LuaSandbox::new()?);
    let api = EngineApi::new();

    // Shared exchange between Lua callbacks and engine update
    struct ScriptExchange {
        transforms: Vec<f64>,         // v2 flat array (scratch, reused)
        transforms_dirty: bool,
        sprites: Vec<SpriteV2>,       // parsed sprites
        textures: Vec<(u32, String)>, // queued texture loads (id, path)
    }
    impl Default for ScriptExchange {
        fn default() -> Self {
            Self { transforms: Vec::with_capacity(1024), transforms_dirty: false, sprites: Vec::with_capacity(1024), textures: Vec::new() }
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
    // Window size shared with Lua window_size()
    let window_size = Arc::new(Mutex::new((1024u32, 768u32)));

    // Create window early to access input handle for providers
    let mut window = EngineWindow::new();

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
        // Input provider closure builds an InputSnapshot from EngineWindow input
        let input_handle = window.input_handle();
        let input_provider = Rc::new(move || {
            let mut snap = engine_scripting::api::InputSnapshot::new();
            if let Ok(inp) = input_handle.lock() {
                snap.mouse_x = inp.mouse_x;
                snap.mouse_y = inp.mouse_y;
                for k in inp.keys.iter() { snap.keys.insert(k.clone(), true); }
                for b in inp.mouse_buttons.iter() { snap.mouse_buttons.insert(b.clone(), true); }
            }
            snap
        });
        api.setup_engine_namespace_with_sinks_and_metrics(
            sandbox.lua(),
            set_transforms_cb,
            submit_sprites_cb,
            hud_provider,
            load_texture_cb,
            input_provider,
            {
                let ws = window_size.clone();
                Rc::new(move || {
                    if let Ok(v) = ws.lock() { *v } else { (1024,768) }
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
        let mut sprites_scratch: Vec<SpriteData> = Vec::with_capacity(1024);
        window.set_script_on_update(move |dt, state| {
            // Advance engine time for Lua view
            api_for_update.update_time(dt);

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
                if ex.transforms_dirty {
                    if let Err(e) = state.set_transforms_from_slice(&ex.transforms) {
                        tracing::error!("Failed to set transforms: {}", e);
                    }
                    ex.transforms_dirty = false;
                }
                if !ex.sprites.is_empty() {
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
        window.set_on_end_frame(move |state, metrics| {
            if let Ok(mut m) = hm.lock() {
                m.cpu_frame_ms = metrics.current_metrics().cpu_frame_ms;
                m.ffi_calls = state.get_ffi_calls_this_frame();
                m.sprites_submitted = state.get_sprites().len() as u32;
            }
            // Update window size from engine_state
            let (w,h) = state.window_size();
            if let Ok(mut wh) = ws_upd.lock() { *wh = (w,h); }
        });
    }

    window.run()?;

    Ok(())
}
