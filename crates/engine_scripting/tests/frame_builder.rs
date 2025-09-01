use engine_scripting::api::{EngineApi, InputSnapshot, SpriteV2};
use mlua::Lua;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn frame_builder_commit_hits_sinks() {
    let lua = Lua::new();
    let api = EngineApi::new();

    let cap_transforms: Rc<RefCell<Option<Vec<f64>>>> = Rc::new(RefCell::new(None));
    let cap_sprites: Rc<RefCell<Vec<SpriteV2>>> = Rc::new(RefCell::new(Vec::new()));
    let st_cap = cap_transforms.clone();
    let sp_cap = cap_sprites.clone();

    api
        .setup_engine_namespace_with_sinks_and_metrics(
            &lua,
            Rc::new(move |slice| {
                *st_cap.borrow_mut() = Some(slice.to_vec());
            }),
            None,
            Rc::new(move |sprites| {
                *sp_cap.borrow_mut() = sprites.to_vec();
            }),
            None,
            Rc::new(|| (0.0, 0, 0)),
            Rc::new(|_, _| {}),
            Rc::new(|| InputSnapshot::default()),
            Rc::new(|| (640, 480)),
            Rc::new(|_| {}),
            Rc::new(|_,_,_,_| {}),
            Rc::new(|_| {}),
        )
        .unwrap();

    let script = r#"
        engine.units.set_pixels_per_unit(64)
        local T = engine.create_transform_buffer(2)
        local S = engine.create_sprite_buffer(2)
        local fb = engine.frame_builder(T, S)
        local e = engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        fb:transform_px(1, e, 32, 48, 0.0, 64, 64)
        fb:sprite_tex(1, e, tex, 0.0,0.0,1.0,1.0, 1,1,1,1)
        fb:commit()
    "#;
    lua.load(script).exec().unwrap();

    let tr = cap_transforms.borrow().clone().unwrap();
    assert_eq!(tr.len(), 6);
    assert!((tr[4] - 1.0).abs() < 1e-9); // sx = 64/64
    assert!((tr[5] - 1.0).abs() < 1e-9);

    let sp = cap_sprites.borrow();
    assert_eq!(sp.len(), 1);
    assert!((sp[0].u0 - 0.0).abs() < 1e-6);
    assert!((sp[0].u1 - 1.0).abs() < 1e-6);
}

#[test]
fn frame_builder_transform_and_color() {
    let lua = Lua::new();
    let api = EngineApi::new();
    let captured: Rc<RefCell<Vec<SpriteV2>>> = Rc::new(RefCell::new(Vec::new()));
    let cap2 = captured.clone();
    api
        .setup_engine_namespace_with_sinks_and_metrics(
            &lua,
            Rc::new(|_| {}),
            None,
            Rc::new(move |sprites: &[SpriteV2]| {
                *cap2.borrow_mut() = sprites.to_vec();
            }),
            None,
            Rc::new(|| (0.0, 0, 0)),
            Rc::new(|_, _| {}),
            Rc::new(|| InputSnapshot::default()),
            Rc::new(|| (800, 600)),
            Rc::new(|_| {}),
            Rc::new(|_,_,_,_| {}),
            Rc::new(|_| {}),
        )
        .unwrap();

    let script = r#"
        engine.units.set_pixels_per_unit(64)
        local T = engine.create_transform_buffer(1)
        local S = engine.create_sprite_buffer(1)
        local fb = engine.frame_builder(T, S)
        local e = engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        fb:transform(1, e, 1.0, 2.0, 0.0, 3.0, 4.0)
        fb:sprite_tex(1, e, tex, 0.0,0.0,1.0,1.0, 0.1,0.2,0.3,0.4)
        fb:sprite_color(1, 0.9,0.8,0.7,0.6)
        fb:commit()
    "#;
    lua.load(script).exec().unwrap();

    let sp = captured.borrow();
    assert_eq!(sp.len(), 1);
    assert!((sp[0].r - 0.9).abs() < 1e-6);
    assert!((sp[0].g - 0.8).abs() < 1e-6);
}

#[test]
fn hud_printf_callable() {
    let lua = Lua::new();
    let api = EngineApi::new();
    let messages: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let msgs2 = messages.clone();
    api
        .setup_engine_namespace_with_sinks_and_metrics(
            &lua,
            Rc::new(|_| {}),
            None,
            Rc::new(|_| {}),
            None,
            Rc::new(|| (0.0, 0, 0)),
            Rc::new(|_, _| {}),
            Rc::new(|| InputSnapshot::default()),
            Rc::new(|| (800, 600)),
            Rc::new(move |msg: String| msgs2.borrow_mut().push(msg)),
            Rc::new(|_,_,_,_| {}),
            Rc::new(|_| {}),
        )
        .unwrap();
    lua.load("engine.hud_printf('hello HUD')").exec().unwrap();
    let got = messages.borrow();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0], "hello HUD");
}
