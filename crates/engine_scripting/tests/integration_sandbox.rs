use engine_scripting::api::EngineApi;
use engine_scripting::sandbox::LuaSandbox;

#[test]
fn load_script_env_and_call_functions() {
    let sandbox = LuaSandbox::new().unwrap();
    // Install engine namespace so scripts can reference engine.*
    let api = EngineApi::new();
    api.setup_engine_namespace(sandbox.lua()).unwrap();

    let script = r#"
        function on_start()
            assert(engine.api_version == 1)
            engine.log("info", "on_start ok")
        end
        function on_update(dt)
            return dt
        end
    "#;
    sandbox.load_script(script, "test.lua").unwrap();
    // Should find and call both functions from the env
    sandbox.call_function::<(), ()>("on_start", ()).unwrap();
    let v: f64 = sandbox.call_function("on_update", (0.5f64,)).unwrap();
    assert!((v - 0.5).abs() < 1e-9);
}

#[test]
fn require_is_blocked_in_sandbox() {
    let sandbox = LuaSandbox::new().unwrap();
    let script = r#"require("foo")"#;
    let err = sandbox.load_script(script, "bad.lua").unwrap_err();
    let s = format!("{}", err);
    // Either require is nil or replaced with erroring function
    assert!(s.contains("require") || s.contains("nil"));
}

#[test]
fn missing_function_reports_error() {
    let sandbox = LuaSandbox::new().unwrap();
    let api = EngineApi::new();
    api.setup_engine_namespace(sandbox.lua()).unwrap();
    let script = r#"function only_one() return 42 end"#;
    sandbox.load_script(script, "one.lua").unwrap();
    let err = sandbox
        .call_function::<(), ()>("does_not_exist", ())
        .unwrap_err();
    assert!(format!("{}", err).contains("not found"));
}

