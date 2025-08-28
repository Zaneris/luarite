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
    #[derive(Default)]
    struct ScriptExchange {
        transforms: Option<Vec<f64>>, // v2 flat array
        sprites: Vec<SpriteV2>,       // parsed sprites
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

    // Install engine namespace with sinks that fill the exchange
    {
        let ex1 = exchange.clone();
        let set_transforms_cb = Rc::new(move |transforms: Vec<f64>| {
            if let Ok(mut ex) = ex1.lock() {
                ex.transforms = Some(transforms);
            }
        });
        let ex2 = exchange.clone();
        let submit_sprites_cb = Rc::new(move |sprites: Vec<SpriteV2>| {
            if let Ok(mut ex) = ex2.lock() {
                ex.sprites.clear();
                ex.sprites.extend(sprites);
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
        api.setup_engine_namespace_with_sinks_and_metrics(
            sandbox.lua(),
            set_transforms_cb,
            submit_sprites_cb,
            hud_provider,
        )?;
    }

    // Load the main game script
    sandbox.load_script(include_str!("../../scripts/game.lua"), "game.lua")?;

    let mut window = EngineWindow::new();

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
        window.set_script_on_update(move |dt, state| {
            // Advance engine time for Lua view
            api_for_update.update_time(dt);

            // Call Lua on_update(dt)
            if let Err(e) = sandbox_for_update.call_function::<(f64,), ()>("on_update", (dt,)) {
                tracing::error!("on_update error: {}", e);
            }

            // Drain exchange into engine state
            if let Ok(mut ex) = exchange_for_update.lock() {
                if let Some(transforms) = ex.transforms.take() {
                    if let Err(e) = state.set_transforms(transforms) {
                        tracing::error!("Failed to set transforms: {}", e);
                    }
                }
                if !ex.sprites.is_empty() {
                    let mut sprites: Vec<SpriteData> = Vec::with_capacity(ex.sprites.len());
                    for s in ex.sprites.drain(..) {
                        sprites.push(SpriteData {
                            entity_id: s.entity_id,
                            texture_id: s.texture_id,
                            uv: [s.u0, s.v0, s.u1, s.v1],
                            color: [s.r, s.g, s.b, s.a],
                        });
                    }
                    if let Err(e) = state.submit_sprites(sprites) {
                        tracing::error!("Failed to submit sprites: {}", e);
                    }
                }
            }
        });
    }

    // Feed HUD metrics from the engine after each frame
    {
        let hm = hud_metrics.clone();
        window.set_on_end_frame(move |state, metrics| {
            if let Ok(mut m) = hm.lock() {
                m.cpu_frame_ms = metrics.current_metrics().cpu_frame_ms;
                m.ffi_calls = state.get_ffi_calls_this_frame();
                m.sprites_submitted = state.get_sprites().len() as u32;
            }
        });
    }

    window.run()?;

    Ok(())
}
