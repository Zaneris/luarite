use engine_core::offscreen::OffscreenRenderer;
use engine_core::state::{EngineState, SpriteData};
use engine_scripting::api::{EngineApi, InputSnapshot};
use mlua::Lua;
use std::cell::RefCell;
use std::rc::Rc;

// Reproduces the host typed-sink flow: Lua fills typed buffers, host drains via zero-copy swaps.
#[test]
fn host_drain_swaps_typed_buffers_and_renders() {
    let lua = Lua::new();
    let api = EngineApi::new();
    let mut state = EngineState::new();
    state.set_window_size(256, 256);

    // Minimal exchange like the host uses
    struct Exchange {
        t_buf: Option<(Rc<RefCell<Vec<f32>>>, usize, usize)>,
        s_buf: Option<(Rc<RefCell<Vec<SpriteData>>>, usize, usize)>,
    }
    let ex = Rc::new(RefCell::new(Exchange { t_buf: None, s_buf: None }));

    // Wire sinks similar to host main
    let st_f64 = Rc::new(|_slice: &[f64]| {});
    let ex1 = ex.clone();
    let st_f32 = Some(Rc::new(move |rc: Rc<RefCell<Vec<f32>>>, rows: usize, cap: usize| {
        ex1.borrow_mut().t_buf = Some((rc.clone(), rows, cap));
    }) as Rc<dyn Fn(Rc<RefCell<Vec<f32>>>, usize, usize)>);
    let sp_v2 = Rc::new(|_sprites: &[engine_scripting::api::SpriteV2]| {});
    let ex2 = ex.clone();
    let sp_typed = Some(Rc::new(move |rc: Rc<RefCell<Vec<SpriteData>>>, rows: usize, cap: usize| {
        ex2.borrow_mut().s_buf = Some((rc.clone(), rows, cap));
    }) as Rc<dyn Fn(Rc<RefCell<Vec<SpriteData>>>, usize, usize)>);

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
    )
    .unwrap();

    // Lua builds one magenta quad at center using typed buffers
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

    // Host drain: swap typed buffers into EngineState
    let mut exm = ex.borrow_mut();
    if let Some((rc, rows, cap)) = exm.t_buf.take() {
        let mut v = rc.borrow_mut();
        state.swap_transform_buffer_with_len(&mut v, rows * 6);
        v.resize(cap * 6, 0.0);
    }
    if let Some((rcs, rows, cap)) = exm.s_buf.take() {
        let mut v = rcs.borrow_mut();
        // New path: swap into back, promote to front, then restore Lua vec
        state.swap_typed_sprites_into_back(&mut v, rows);
        state.promote_sprites_back_to_front();
        state.restore_lua_sprite_vec(&mut v, cap);
    }

    // Render offscreen and assert center pixel is magenta-ish
    let rdr = pollster::block_on(OffscreenRenderer::new(256, 256)).unwrap();
    let rgba = rdr.render_state_to_rgba(&state).unwrap();
    let x = 128u32;
    let y = 128u32;
    let idx = ((y * 256 + x) * 4) as usize;
    let r = rgba[idx] as i32;
    let g = rgba[idx + 1] as i32;
    let b = rgba[idx + 2] as i32;
    assert!(r > 200 && g < 40 && b > 200, "unexpected center color {},{},{}", r, g, b);
}
