use engine_scripting::api::{EngineApi, InputSnapshot, SpriteV2};
use mlua::Lua;
use std::cell::RefCell;
use std::rc::Rc;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

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
            None,
            Rc::new(|_| {}),
            None,
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
fn transform_buffer_set_px_converts_units() {
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
            None,
            Rc::new(|_| {}),
            None,
        )
        .unwrap();

    let script = r#"
        engine.units.set_pixels_per_unit(32)
        local T = engine.create_transform_buffer(1)
        local e = engine.create_entity()
        T:set_px(1, e, 0, 0, 0.0, 64, 96)
        engine.set_transforms(T)
    "#;
    lua.load(script).exec().unwrap();

    let tr = captured.borrow().clone().unwrap();
    // sx=64/32=2, sy=96/32=3
    assert!((tr[4] - 2.0).abs() < 1e-9);
    assert!((tr[5] - 3.0).abs() < 1e-9);
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
            None,
            Rc::new(move |sprites| {
                *cap2.borrow_mut() = sprites.to_vec();
            }),
            None,
            Rc::new(|| (0.0, 0, 0)),
            Rc::new(|_, _| {}),
            Rc::new(|| InputSnapshot::default()),
            Rc::new(|| (800, 600)),
            Rc::new(|_| {}),
            Rc::new(|_,_,_,_| {}),
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
fn atlas_load_parses_json() {
    let lua = Lua::new();
    let api = EngineApi::new();
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
            Rc::new(|_| {}),
            Rc::new(|_,_,_,_| {}),
        )
        .unwrap();
    // Write a temporary atlas JSON
    let mut p = PathBuf::from(std::env::temp_dir());
    p.push(format!("atlas_test_{}.json", std::process::id()));
    let mut f = fs::File::create(&p).unwrap();
    write!(
        f,
        "{{\n  \"width\": 128, \"height\": 64, \"frames\": {{ \"ball\": {{ \"x\": 0, \"y\": 0, \"w\": 16, \"h\": 16 }} }}\n}}"
    )
    .unwrap();
    drop(f);
    let script = format!("return engine.atlas_load('dummy.png','{}')", p.display());
    let val: mlua::Value = lua.load(&script).eval().unwrap();
    // Should be a userdata Atlas when JSON is present
    match val { mlua::Value::UserData(_) => {}, _ => panic!("expected Atlas user data") }
}
