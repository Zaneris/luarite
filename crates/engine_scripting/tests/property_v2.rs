use engine_scripting::api::{EngineApi, SpriteV2};
use mlua::Lua;
use proptest::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

// Helper: truncate length to nearest multiple of 6
fn truncate_to_stride6(mut v: Vec<f64>) -> Vec<f64> {
    let rem = v.len() % 6;
    if rem != 0 {
        v.truncate(v.len() - rem);
    }
    v
}

proptest! {
    #[test]
    fn base_api_valid_stride_passes(mut vals in proptest::collection::vec((-1e3f64..1e3f64), 0..120)) {
        let lua = Lua::new();
        let api = EngineApi::new();
        api.setup_engine_namespace(&lua).unwrap();
        let engine_tbl: mlua::Table = lua.globals().get("engine").unwrap();
        let func: mlua::Function = engine_tbl.get("set_transforms").unwrap();

        let v = truncate_to_stride6(vals); // length now multiple of 6 (including 0)
        // Call should succeed for valid stride
        let _ = func.call::<()>(v).unwrap();
    }

    #[test]
    fn base_api_invalid_stride_fails(mut vals in proptest::collection::vec((-1e3f64..1e3f64), 1..120)) {
        // Ensure length is not multiple of 6
        if vals.len() % 6 == 0 {
            vals.push(0.0);
        }

        let lua = Lua::new();
        let api = EngineApi::new();
        api.setup_engine_namespace(&lua).unwrap();
        let engine_tbl: mlua::Table = lua.globals().get("engine").unwrap();
        let func: mlua::Function = engine_tbl.get("set_transforms").unwrap();

        let err = func.call::<()>(vals).unwrap_err();
        let s = format!("{}", err);
        prop_assert!(s.contains("ARG_ERROR: set_transforms stride mismatch"));
    }
}

proptest! {
    #[test]
    fn sinks_valid_stride_passes_and_hits_sink(mut vals in proptest::collection::vec((-1e3f64..1e3f64), 0..120)) {
        let lua = Lua::new();
        let api = EngineApi::new();

        // capture sink length
        let cap_len: Rc<RefCell<usize>> = Rc::new(RefCell::new(usize::MAX));
        let cap2 = cap_len.clone();

        api.setup_engine_namespace_with_sinks(
            &lua,
            Rc::new(move |slice| { *cap2.borrow_mut() = slice.len(); }),
            Rc::new(|_: &[SpriteV2]| {}),
        ).unwrap();

        // Build Lua table with length multiple of 6
        let v = truncate_to_stride6(vals);
        let t = lua.create_table().unwrap();
        for (i, x) in v.iter().enumerate() { t.raw_set(i+1, *x).unwrap(); }

        let engine_tbl: mlua::Table = lua.globals().get("engine").unwrap();
        let func: mlua::Function = engine_tbl.get("set_transforms").unwrap();
        func.call::<()>(t).unwrap();
        let got = *cap_len.borrow();
        prop_assert_eq!(got, v.len());
    }

    #[test]
    fn sinks_invalid_stride_fails(mut vals in proptest::collection::vec((-1e3f64..1e3f64), 1..120)) {
        if vals.len() % 6 == 0 { vals.push(0.0); }

        let lua = Lua::new();
        let api = EngineApi::new();
        api.setup_engine_namespace_with_sinks(&lua, Rc::new(|_| {}), Rc::new(|_| {})).unwrap();

        let t = lua.create_table().unwrap();
        for (i, x) in vals.iter().enumerate() { t.raw_set(i+1, *x).unwrap(); }

        let engine_tbl: mlua::Table = lua.globals().get("engine").unwrap();
        let func: mlua::Function = engine_tbl.get("set_transforms").unwrap();
        let err = func.call::<()>(t).unwrap_err();
        let s = format!("{}", err);
        prop_assert!(s.contains("ARG_ERROR: set_transforms stride mismatch"));
    }
}
