use engine_core::offscreen::OffscreenRenderer;
use engine_core::state::EngineState;
use engine_scripting::api::{EngineApi, InputSnapshot};
use mlua::Lua;
use std::cell::RefCell;
use std::rc::Rc;

// Ensure that sprites submitted once persist across frames when only transforms are updated.
#[test]
fn sprites_persist_across_frames_without_resubmission() {
    let lua = Lua::new();
    let api = EngineApi::new();
    let mut state = EngineState::new();
    state.set_window_size(256, 256);

    // Simple host-like exchange
    struct Exchange {
        t_buf: Option<(Rc<RefCell<Vec<f32>>>, usize, usize)>,
        s_buf: Option<(Rc<RefCell<Vec<engine_core::state::SpriteData>>>, usize, usize)>,
        v2_sprites: Vec<engine_scripting::api::SpriteV2>,
    }
    let ex = Rc::new(RefCell::new(Exchange { t_buf: None, s_buf: None, v2_sprites: Vec::new() }));

    // Wire sinks
    let ex1 = ex.clone();
    let st_f32 = Some(Rc::new(move |rc: Rc<RefCell<Vec<f32>>>, rows: usize, cap: usize| {
        ex1.borrow_mut().t_buf = Some((rc.clone(), rows, cap));
    }) as Rc<dyn Fn(Rc<RefCell<Vec<f32>>>, usize, usize)>);
    let ex2 = ex.clone();
    let sp_typed = Some(Rc::new(move |rc: Rc<RefCell<Vec<engine_core::state::SpriteData>>>, rows: usize, cap: usize| {
        ex2.borrow_mut().s_buf = Some((rc.clone(), rows, cap));
    }) as Rc<dyn Fn(Rc<RefCell<Vec<engine_core::state::SpriteData>>>, usize, usize)>);

    api.setup_engine_namespace_with_sinks_and_metrics(
        &lua,
        Rc::new(|_| {}),
        st_f32,
        Rc::new({
            let exv = ex.clone();
            move |v: &[engine_scripting::api::SpriteV2]| {
                exv.borrow_mut().v2_sprites.clear();
                exv.borrow_mut().v2_sprites.extend_from_slice(v);
            }
        }),
        sp_typed,
        Rc::new(|| (0.0, 0, 0)),
        Rc::new(|_, _| {}),
        Rc::new(|| InputSnapshot::default()),
        Rc::new(|| (256, 256)),
        Rc::new(|_| {}),
    )
    .unwrap();

    // Frame 1: set sprites and transforms
    let script_frame1 = r#"
        engine.units.set_pixels_per_unit(64)
        if not T then
            T = engine.create_transform_buffer(1)
            S = engine.create_sprite_buffer(1)
            E = engine.create_entity()
            local tex = engine.load_texture("dummy.png")
            S:set_tex(1, E, tex)
            S:set_uv_rect(1, 0.0,0.0,1.0,1.0)
            S:set_color(1, 1.0,0.0,1.0,1.0)
        end
        T:set_px(1, E, 128, 128, 0.0, 64, 64)
        engine.set_transforms(T)
        engine.submit_sprites(S)
    "#;
    lua.load(script_frame1).exec().unwrap();
    {
        let mut exm = ex.borrow_mut();
        // drain transforms (swap)
        let (rc, rows, cap) = exm.t_buf.take().unwrap();
        let mut v = rc.borrow_mut();
        state.swap_transform_buffer_with_len(&mut v, rows * 6);
        v.resize(cap * 6, 0.0);
        // drain sprites (copy)
        let (rcs, rows, _cap) = exm.s_buf.take().unwrap();
        let v = rcs.borrow();
        let mut scratch = Vec::new();
        scratch.extend_from_slice(&v[..rows]);
        state.append_sprites(&mut scratch).unwrap();
    }
    // Render after frame 1
    let rdr = pollster::block_on(OffscreenRenderer::new(256, 256)).unwrap();
    let rgba1 = rdr.render_state_to_rgba(&state).unwrap();

    // Frame 2: move only transforms; do NOT resubmit sprites
    let script_frame2 = r#"
        T:set_px(1, E, 120, 120, 0.0, 64, 64)
        engine.set_transforms(T)
    "#;
    lua.load(script_frame2).exec().unwrap();
    {
        let mut exm = ex.borrow_mut();
        // drain transforms only
        let (rc, rows, cap) = exm.t_buf.take().unwrap();
        let mut v = rc.borrow_mut();
        state.swap_transform_buffer_with_len(&mut v, rows * 6);
        v.resize(cap * 6, 0.0);
        // no typed sprites this frame
    }
    // Render after frame 2
    let rgba2 = rdr.render_state_to_rgba(&state).unwrap();

    // Assert both frames had a visible magenta-ish pixel at their respective centers
    let sample = |img: &Vec<u8>, x: u32, y: u32| {
        let idx = ((y * 256 + x) * 4) as usize;
        (img[idx] as i32, img[idx + 1] as i32, img[idx + 2] as i32)
    };
    let (r1, g1, b1) = sample(&rgba1, 128, 128);
    assert!(r1 > 200 && g1 < 40 && b1 > 200);
    let (r2, g2, b2) = sample(&rgba2, 120, 120);
    assert!(r2 > 200 && g2 < 40 && b2 > 200);
}

