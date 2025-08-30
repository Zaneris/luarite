use engine_core::offscreen::OffscreenRenderer;
use engine_core::state::EngineState;
use engine_scripting::api::{EngineApi, InputSnapshot, SpriteV2};
use mlua::Lua;
use std::cell::RefCell;
use std::rc::Rc;

// Ensure that when both typed and V2 submissions occur in the same frame, typed wins.
#[test]
fn typed_submission_takes_precedence_over_v2() {
    let lua = Lua::new();
    let api = EngineApi::new();
    let mut state = EngineState::new();
    state.set_window_size(256, 256);

    struct Exchange {
        t_buf: Option<(Rc<RefCell<Vec<f32>>>, usize, usize)>,
        s_buf_typed: Option<(Rc<RefCell<Vec<engine_core::state::SpriteData>>>, usize, usize)>,
        s_v2: Vec<SpriteV2>,
    }
    let ex = Rc::new(RefCell::new(Exchange { t_buf: None, s_buf_typed: None, s_v2: Vec::new() }));

    // Wire sinks
    let ex1 = ex.clone();
    let st_f32 = Some(Rc::new(move |rc: Rc<RefCell<Vec<f32>>>, rows: usize, cap: usize| {
        ex1.borrow_mut().t_buf = Some((rc.clone(), rows, cap));
    }) as Rc<dyn Fn(Rc<RefCell<Vec<f32>>>, usize, usize)>);
    let ex2 = ex.clone();
    let sp_typed = Some(Rc::new(move |rc: Rc<RefCell<Vec<engine_core::state::SpriteData>>>, rows: usize, cap: usize| {
        ex2.borrow_mut().s_buf_typed = Some((rc.clone(), rows, cap));
    }) as Rc<dyn Fn(Rc<RefCell<Vec<engine_core::state::SpriteData>>>, usize, usize)>);
    let ex3 = ex.clone();
    let sp_v2 = Rc::new(move |v: &[SpriteV2]| {
        ex3.borrow_mut().s_v2.clear();
        ex3.borrow_mut().s_v2.extend_from_slice(v);
    });

    api.setup_engine_namespace_with_sinks_and_metrics(
        &lua,
        Rc::new(|_| {}),
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

    // In the same frame, submit typed (magenta) and then V2 (green) for the same entity
    let script = r#"
        engine.units.set_pixels_per_unit(64)
        local T = engine.create_transform_buffer(1)
        local S = engine.create_sprite_buffer(1)
        local e = engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        T:set_px(1, e, 128, 128, 0.0, 64, 64)
        S:set_tex(1, e, tex)
        S:set_uv_rect(1, 0.0, 0.0, 1.0, 1.0)
        S:set_color(1, 1.0, 0.0, 1.0, 1.0) -- magenta
        engine.set_transforms(T)
        engine.submit_sprites(S)         -- typed path
        -- v2 fallback attempt (green) should be ignored by host precedence
        local s = { e, tex, 0.0,0.0,1.0,1.0, 0.0,1.0,0.0,1.0 }
        engine.submit_sprites(s)
    "#;
    lua.load(script).exec().unwrap();

    // Host drain with precedence: if typed present, copy typed and ignore v2
    {
        let mut exm = ex.borrow_mut();
        let (rc, rows, cap) = exm.t_buf.take().unwrap();
        let mut v = rc.borrow_mut();
        state.swap_transform_buffer_with_len(&mut v, rows * 6);
        v.resize(cap * 6, 0.0);
        if let Some((rcs, rows, _cap)) = exm.s_buf_typed.take() {
            let v = rcs.borrow();
            let mut scratch = Vec::new();
            scratch.extend_from_slice(&v[..rows]);
            state.append_sprites(&mut scratch).unwrap();
        } else if !exm.s_v2.is_empty() {
            // This branch should not run in this test
            let mut scratch = Vec::new();
            for s in exm.s_v2.drain(..) {
                scratch.push(engine_core::state::SpriteData { entity_id: s.entity_id, texture_id: s.texture_id, uv: [s.u0,s.v0,s.u1,s.v1], color: [s.r,s.g,s.b,s.a]});
            }
            state.append_sprites(&mut scratch).unwrap();
        }
    }

    // Render and assert magenta (typed) wins over green (v2)
    let rdr = pollster::block_on(OffscreenRenderer::new(256, 256)).unwrap();
    let rgba = rdr.render_state_to_rgba(&state).unwrap();
    let idx = ((128u32 * 256 + 128u32) * 4) as usize;
    let r = rgba[idx] as i32;
    let g = rgba[idx + 1] as i32;
    let b = rgba[idx + 2] as i32;
    assert!(r > 200 && g < 40 && b > 200, "typed should win over v2; got {},{},{}", r, g, b);
}
