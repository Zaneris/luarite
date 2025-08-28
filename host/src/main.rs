use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber;
use engine_core::window::EngineWindow;
use engine_scripting::sandbox::LuaSandbox;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    info!("Luarite Engine starting...");

    // Test Lua sandbox first
    test_lua_sandbox()?;

    let window = EngineWindow::new();
    window.run()?;

    Ok(())
}

fn test_lua_sandbox() -> Result<()> {
    info!("Testing Lua sandbox...");
    
    let sandbox = LuaSandbox::new()?;
    info!("Lua sandbox initialized successfully");
    
    // Test script to verify sandbox security
    let test_script = r#"
        -- Test script to verify sandbox works and is secure
        local x = math.pi
        local s = string.upper("hello")
        
        function test_function()
            return "Lua sandbox is working! π = " .. tostring(x) .. ", HELLO = " .. s
        end
        
        function test_security()
            -- These should be blocked by the sandbox
            local blocked_funcs = { "io", "os", "require", "dofile", "loadfile", "print" }
            local blocked_results = {}
            
            for _, func_name in ipairs(blocked_funcs) do
                local func = _G[func_name]
                if func == nil then
                    blocked_results[func_name] = "blocked"
                else
                    blocked_results[func_name] = "DANGER: not blocked!"
                end
            end
            
            return blocked_results
        end
    "#;
    
    sandbox.load_script(test_script, "security_test.lua")?;
    
    // Test basic functionality
    let result: String = sandbox.call_function("test_function", ())?;
    info!("Lua test result: {}", result);
    
    // Test security (dangerous functions should be blocked)
    let security_results: std::collections::HashMap<String, String> = 
        sandbox.call_function("test_security", ())?;
    
    for (func, status) in security_results {
        if status.contains("DANGER") {
            tracing::error!("Security failure: {} is {}", func, status);
        } else {
            tracing::debug!("Security check: {} is {}", func, status);
        }
    }
    
    info!("Lua memory usage: {:.2} MB", sandbox.get_memory_usage());
    info!("Lua sandbox security test completed");
    
    Ok(())
}