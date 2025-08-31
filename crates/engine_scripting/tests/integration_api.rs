use engine_scripting::api::{EngineApi, InputSnapshot};
use mlua::Lua;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn sinks_integration_end_to_end() {
    let lua = Lua::new();
    let api = EngineApi::new();

    // Capture sinks
    let cap_transforms: Rc<RefCell<Option<Vec<f64>>>> = Rc::new(RefCell::new(None));
    let cap_sprites: Rc<RefCell<Vec<engine_scripting::api::SpriteV2>>> =
        Rc::new(RefCell::new(Vec::new()));
    let st_cap = cap_transforms.clone();
    let sp_cap = cap_sprites.clone();

    let metrics = Rc::new(|| (0.0, 0, 0));
    let tex_events: Rc<RefCell<Vec<(String, u32)>>> = Rc::new(RefCell::new(Vec::new()));
    let tex_ev2 = tex_events.clone();

    api.setup_engine_namespace_with_sinks_and_metrics(
        &lua,
        Rc::new(move |slice| {
            *st_cap.borrow_mut() = Some(slice.to_vec());
        }),
        None,
        Rc::new(move |sprites| {
            *sp_cap.borrow_mut() = sprites.to_vec();
        }),
        None,
        metrics,
        Rc::new(move |path, id| tex_ev2.borrow_mut().push((path, id))),
        Rc::new(|| InputSnapshot::default()),
        Rc::new(|| (1024, 768)),
        Rc::new(|_| {}),
        Rc::new(|_,_,_,_| {}),
        Rc::new(|_| {}),
    )
    .unwrap();

    // Script that calls set_transforms and submit_sprites
    let script = r#"
        local e = engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        local t = { e, 1.0, 2.0, 0.0, 1.0, 1.0 }
        local s = { e, tex, 0.0, 0.0, 1.0, 1.0, 1.0, 0.5, 0.25, 1.0 }
        engine.set_transforms(t)
        engine.submit_sprites(s)
    "#;
    lua.load(script).exec().unwrap();

    // Validate captures
    let tr = cap_transforms.borrow().clone().unwrap();
    assert_eq!(tr.len(), 6);
    assert_eq!(tr[1], 1.0);
    assert_eq!(tr[2], 2.0);

    let sp = cap_sprites.borrow();
    assert_eq!(sp.len(), 1);
    assert!((sp[0].r - 1.0).abs() < 1e-6);
    assert!((sp[0].g - 0.5).abs() < 1e-6);
    assert!((sp[0].b - 0.25).abs() < 1e-6);

    // Texture load callback was invoked
    let tex = tex_events.borrow();
    assert_eq!(tex.len(), 1);
    assert_eq!(tex[0].0, "dummy.png");
}

#[test]
fn input_provider_is_accessible_from_lua() {
    let lua = Lua::new();
    let api = EngineApi::new();

    // Provide an input snapshot with KeyA = true
    let input_provider = Rc::new(|| {
        let mut snap = InputSnapshot::default();
        snap.keys.insert("KeyA".to_string(), true);
        snap
    });

    api.setup_engine_namespace_with_sinks_and_metrics(
        &lua,
        Rc::new(|_| {}),
        None,
        Rc::new(|_| {}),
        None,
        Rc::new(|| (0.0, 0, 0)),
        Rc::new(|_, _| {}),
        input_provider,
        Rc::new(|| (1024, 768)),
        Rc::new(|_| {}),
        Rc::new(|_,_,_,_| {}),
        Rc::new(|_| {}),
    )
    .unwrap();

    let script = r#"
        function test_input()
            local inp = engine.get_input()
            return inp:get_key("KeyA")
        end
    "#;
    lua.load(script).exec().unwrap();
    let globals = lua.globals();
    let f: mlua::Function = globals.get("test_input").unwrap();
    let ok: bool = f.call::<bool>(()).unwrap();
    assert!(ok);
}

#[test]
fn metrics_provider_roundtrip() {
    let lua = Lua::new();
    let api = EngineApi::new();

    // Provider returns specific values
    let metrics = Rc::new(|| (12.34f64, 7u32, 2u32));

    api.setup_engine_namespace_with_sinks_and_metrics(
        &lua,
        Rc::new(|_| {}),
        None,
        Rc::new(|_| {}),
        None,
        metrics,
        Rc::new(|_, _| {}),
        Rc::new(|| InputSnapshot::default()),
        Rc::new(|| (1024, 768)),
        Rc::new(|_| {}),
        Rc::new(|_,_,_,_| {}),
        Rc::new(|_| {}),
    )
    .unwrap();

    let script = r#"
        function test_metrics()
            local m = engine.get_metrics()
            return m.cpu_frame_ms, m.sprites_submitted, m.ffi_calls
        end
    "#;
    lua.load(script).exec().unwrap();
    let globals = lua.globals();
    let f: mlua::Function = globals.get("test_metrics").unwrap();
    let (cpu, sprites, ffi): (f64, u32, u32) = f.call::<(f64, u32, u32)>(()).unwrap();
    assert!((cpu - 12.34).abs() < 1e-6);
    assert_eq!(sprites, 7);
    assert_eq!(ffi, 2);
}

#[test]
fn rng_seed_is_deterministic() {
    let lua = Lua::new();
    let api = EngineApi::new();
    api.setup_engine_namespace(&lua).unwrap();

    let script = r#"
        function seq()
            engine.seed(42)
            local a = engine.random()
            local b = engine.random()
            return a, b
        end
    "#;
    lua.load(script).exec().unwrap();
    let globals = lua.globals();
    let f: mlua::Function = globals.get("seq").unwrap();
    let (a1,b1): (f64,f64) = f.call::<(f64,f64)>(()).unwrap();
    let (a2,b2): (f64,f64) = f.call::<(f64,f64)>(()).unwrap();
    assert!((a1-a2).abs() < 1e-12 && (b1-b2).abs() < 1e-12);
}

#[test]
fn persistence_roundtrip_lua() {
    let lua = Lua::new();
    let api = EngineApi::new();
    api.setup_engine_namespace(&lua).unwrap();

    let script = r#"
        function do_persist()
            engine.persist("foo", 123.0)
            return engine.restore("foo")
        end
    "#;
    lua.load(script).exec().unwrap();
    let globals = lua.globals();
    let f: mlua::Function = globals.get("do_persist").unwrap();
    let v: f64 = f.call::<f64>(()).unwrap();
    assert!((v - 123.0) < 1e-6);
}
