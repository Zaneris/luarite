use engine_core::offscreen::OffscreenRenderer;
use engine_core::state::EngineState;
use engine_scripting::api::{EngineApi, InputSnapshot};
use mlua::Lua;
use std::cell::RefCell;
use std::rc::Rc;

// Guard against regressions where draining typed sprites empties the Lua buffer
// and a second drain in the same frame clears the engine's sprite list (causing flicker).
#[test]
fn double_drain_same_frame_keeps_sprites() {
    let lua = Lua::new();
    let api = EngineApi::new();
    let mut state = EngineState::new();
    state.set_window_size(256, 256);

    // Minimal exchange like the host uses
    struct Exchange {
        t_buf: Option<(Rc<RefCell<Vec<f32>>>, usize, usize)>,
        s_buf: Option<(Rc<RefCell<Vec<engine_core::state::SpriteData>>>, usize, usize)>,
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
    let sp_typed = Some(Rc::new(move |rc: Rc<RefCell<Vec<engine_core::state::SpriteData>>>, rows: usize, cap: usize| {
        ex2.borrow_mut().s_buf = Some((rc.clone(), rows, cap));
    }) as Rc<dyn Fn(Rc<RefCell<Vec<engine_core::state::SpriteData>>>, usize, usize)>);

    api.setup_engine_namespace_with_sinks_and_metrics(
        &lua,
        st_f64,
        st_f32,
        sp_v2,
        sp_typed,
        Rc::new(|| (0.0, 0, 0)),
        Rc::new(|_, _| {}),
        Rc::new(|| InputSnapshot::default()),
        Rc::new(|| (256, 256)),
        Rc::new(|_| {}),
        Rc::new(|_,_,_,_| {}),
    )
    .unwrap();

    // Lua sets typed buffers once and submits
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

    // Drain transforms once (swap is fine for transforms)
    {
        let mut exm = ex.borrow_mut();
        let (rc, rows, cap) = exm.t_buf.take().expect("no typed transforms");
        let mut v = rc.borrow_mut();
        state.swap_transform_buffer_with_len(&mut v, rows * 6);
        v.resize(cap * 6, 0.0);
    }

    // Intentionally drain typed sprites twice in the same frame.
    // If implementation uses swap/destroy, the second drain would clear sprites.
    for _ in 0..2 {
        let mut exm = ex.borrow_mut();
        let (rcs, rows, _cap) = exm.s_buf.take().expect("no typed sprites");
        let v = rcs.borrow();
        let mut scratch = Vec::new();
        scratch.extend_from_slice(&v[..rows]);
        state.append_sprites(&mut scratch).unwrap();
        // Put back the buffer so the second loop can read it again
        exm.s_buf = Some((rcs.clone(), rows, _cap));
    }

    // Render and assert center pixel is magenta-ish
    let rdr = pollster::block_on(OffscreenRenderer::new(256, 256)).unwrap();
    let rgba = rdr.render_state_to_rgba(&state).unwrap();
    let idx = ((128u32 * 256 + 128u32) * 4) as usize;
    let r = rgba[idx] as i32;
    let g = rgba[idx + 1] as i32;
    let b = rgba[idx + 2] as i32;
    assert!(r > 200 && g < 40 && b > 200, "unexpected center color {},{},{}", r, g, b);
}
