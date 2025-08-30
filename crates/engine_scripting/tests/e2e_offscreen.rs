use engine_core::offscreen::OffscreenRenderer;
use engine_core::state::EngineState;
use engine_scripting::api::{EngineApi, InputSnapshot, SpriteV2};
use mlua::Lua;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn lua_draws_magenta_quad_offscreen() {
    let lua = Lua::new();
    let api = EngineApi::new();
    let mut state = EngineState::new();
    state.set_window_size(256, 256);

    // Sinks: typed transforms and typed sprites update EngineState directly
    let st_f64 = Rc::new(|_slice: &[f64]| {}); // unused in this test
    let st_f32 = Some(Rc::new({
        // Update transforms directly as f32
        let state_ptr: *mut EngineState = &mut state;
        move |rc: Rc<RefCell<Vec<f32>>>, rows: usize, _cap: usize| {
            let buf = rc.borrow();
            let take = rows * 6;
            unsafe {
                (*state_ptr)
                    .set_transforms_from_f32_slice(&buf[..take])
                    .unwrap();
            }
        }
    }) as Rc<dyn Fn(Rc<RefCell<Vec<f32>>>, usize, usize)>);

    // We'll accept V2 sprite fallback (unused) and also typed sprites (preferred)
    let sp_v2 = Rc::new(|_sprites: &[SpriteV2]| {});
    let sp_typed = Some(Rc::new({
        let state_ptr: *mut EngineState = &mut state;
        move |rc: Rc<RefCell<Vec<engine_core::state::SpriteData>>>, rows: usize, cap: usize| {
            let mut v = rc.borrow_mut();
            unsafe {
                (*state_ptr).swap_typed_sprites_into_back(&mut v, rows);
                (*state_ptr).promote_sprites_back_to_front();
                (*state_ptr).restore_lua_sprite_vec(&mut v, cap);
            }
        }
    }) as Rc<dyn Fn(Rc<RefCell<Vec<engine_core::state::SpriteData>>>, usize, usize)>);

    // Metrics/provider closures (not used in this test)
    let metrics = Rc::new(|| (0.0, 0, 0));
    let load_tex = Rc::new(|_path: String, _id: u32| {});
    let input = Rc::new(|| InputSnapshot::default());
    let window_size = Rc::new(|| (256u32, 256u32));
    let hud = Rc::new(|_s: String| {});

    api.setup_engine_namespace_with_sinks_and_metrics(
        &lua,
        st_f64,
        st_f32,
        sp_v2,
        sp_typed,
        metrics,
        load_tex,
        input,
        window_size,
        hud,
        Rc::new({
            let state_ptr: *mut EngineState = &mut state;
            move |r,g,b,a| unsafe { (*state_ptr).set_clear_color(r,g,b,a); }
        }),
    )
    .unwrap();

    // Lua script: one magenta quad at center
    let script = r#"
        engine.units.set_pixels_per_unit(64)
        local T = engine.create_transform_buffer(1)
        local S = engine.create_sprite_buffer(1)
        local e = engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        T:set_px(1, e, 128, 128, 0.0, 64, 64)
        S:set_tex(1, e, tex)
        S:set_uv_rect(1, 0.0, 0.0, 1.0, 1.0)
        S:set_color(1, 1.0, 0.0, 1.0, 1.0)
        engine.set_transforms(T)
        engine.submit_sprites(S)
    "#;
    lua.load(script).exec().unwrap();

    let rdr = pollster::block_on(OffscreenRenderer::new(256, 256)).unwrap();
    let rgba = rdr.render_state_to_rgba(&state).unwrap();
    let x = 128u32; let y = 128u32; let idx = ((y * 256 + x) * 4) as usize;
    let r = rgba[idx] as i32; let g = rgba[idx+1] as i32; let b = rgba[idx+2] as i32;
    assert!(r > 200 && g < 40 && b > 200, "unexpected center color {},{},{}", r,g,b);
}

#[test]
fn lua_sets_background_clear_color() {
    let lua = Lua::new();
    let api = EngineApi::new();
    let mut state = EngineState::new();
    state.set_window_size(64, 64);

    // Minimal sinks; only clear color used
    api.setup_engine_namespace_with_sinks_and_metrics(
        &lua,
        Rc::new(|_| {}),
        None,
        Rc::new(|_| {}),
        None,
        Rc::new(|| (0.0, 0, 0)),
        Rc::new(|_, _| {}),
        Rc::new(|| InputSnapshot::default()),
        Rc::new(|| (64, 64)),
        Rc::new(|_| {}),
        Rc::new({
            let state_ptr: *mut EngineState = &mut state;
            move |r,g,b,a| unsafe { (*state_ptr).set_clear_color(r,g,b,a); }
        }),
    ).unwrap();

    // Script sets clear to red
    lua.load("engine.set_clear_color(1.0, 0.0, 0.0)").exec().unwrap();

    let rdr = pollster::block_on(OffscreenRenderer::new(64, 64)).unwrap();
    let rgba = rdr.render_state_to_rgba(&state).unwrap();
    // Sample center pixel; expect red background (no sprites)
    let idx = ((32u32 * 64 + 32u32) * 4) as usize;
    let r = rgba[idx] as i32; let g = rgba[idx+1] as i32; let b = rgba[idx+2] as i32;
    assert!(r > 200 && g < 40 && b < 40, "unexpected bg color {},{},{}", r, g, b);
}
