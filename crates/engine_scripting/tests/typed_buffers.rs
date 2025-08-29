use engine_scripting::api::{EngineApi, InputSnapshot, SpriteV2};
use mlua::Lua;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn typed_transform_buffer_hits_sink() {
    let lua = Lua::new();
    let api = EngineApi::new();
    let captured: Rc<RefCell<Option<Vec<f64>>>> = Rc::new(RefCell::new(None));
    let cap2 = captured.clone();
    api
        .setup_engine_namespace_with_sinks(
            &lua,
            Rc::new(move |slice| {
                *cap2.borrow_mut() = Some(slice.to_vec());
            }),
            Rc::new(|_| {}),
        )
        .unwrap();

    let script = r#"
        local T = engine.create_transform_buffer(2)
        local e = engine.create_entity()
        T:set(1, e, 10.0, 20.0, 0.0, 1.0, 1.0)
        engine.set_transforms(T)
    "#;
    lua.load(script).exec().unwrap();

    let got = captured.borrow().clone().unwrap();
    assert_eq!(got.len(), 6);
    assert_eq!(got[1], 10.0);
    assert_eq!(got[2], 20.0);
}

#[test]
fn typed_sprite_buffer_hits_sink() {
    let lua = Lua::new();
    let api = EngineApi::new();
    let captured: Rc<RefCell<Vec<SpriteV2>>> = Rc::new(RefCell::new(Vec::new()));
    let cap2 = captured.clone();
    api
        .setup_engine_namespace_with_sinks_and_metrics(
            &lua,
            Rc::new(|_| {}),
            Rc::new(move |sprites| {
                *cap2.borrow_mut() = sprites.to_vec();
            }),
            Rc::new(|| (0.0, 0, 0)),
            Rc::new(|_, _| {}),
            Rc::new(|| InputSnapshot::default()),
            Rc::new(|| (800, 600)),
        )
        .unwrap();

    let script = r#"
        local S = engine.create_sprite_buffer(1)
        local e = engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        S:set(1, e, tex, 0.1,0.2,0.9,0.8, 1.0,0.5,0.25,1.0)
        engine.submit_sprites(S)
    "#;
    lua.load(script).exec().unwrap();

    let got = captured.borrow();
    assert_eq!(got.len(), 1);
    let s = &got[0];
    assert!((s.u0 - 0.1).abs() < 1e-6);
    assert!((s.v1 - 0.8).abs() < 1e-6);
    assert!((s.g - 0.5).abs() < 1e-6);
}

#[test]
fn atlas_load_missing_is_nil() {
    let lua = Lua::new();
    let api = EngineApi::new();
    api
        .setup_engine_namespace_with_sinks_and_metrics(
            &lua,
            Rc::new(|_| {}),
            Rc::new(|_| {}),
            Rc::new(|| (0.0, 0, 0)),
            Rc::new(|_, _| {}),
            Rc::new(|| InputSnapshot::default()),
            Rc::new(|| (800, 600)),
        )
        .unwrap();
    let val: mlua::Value = lua
        .load("return engine.atlas_load('nope.png','missing.json')")
        .eval()
        .unwrap();
    matches!(val, mlua::Value::Nil);
}

