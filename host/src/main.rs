use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber;
use engine_core::window::EngineWindow;
use engine_scripting::sandbox::LuaSandbox;
use engine_scripting::api::EngineApi;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    info!("Luarite Engine starting...");

    // Test Lua sandbox first
    test_lua_sandbox()?;

    info!("API test completed successfully - now starting window");
    
    let window = EngineWindow::new();
    window.run()?;

    Ok(())
}

fn test_lua_sandbox() -> Result<()> {
    info!("Testing Lua sandbox with Engine API...");
    
    let sandbox = LuaSandbox::new()?;
    let api = EngineApi::new();
    
    // Set up the engine namespace
    api.setup_engine_namespace(sandbox.lua())?;
    
    info!("Lua sandbox and Engine API initialized successfully");
    
    // Test script to verify API and security
    let test_script = r#"
        -- Assert API version compatibility
        assert(engine.api_version == 1, "Script requires API version 1, got " .. tostring(engine.api_version))
        
        function test_api()
            -- Test capabilities
            local caps = engine.get_capabilities()
            local max_entities = caps:max_entities()
            
            -- Test entity creation
            local entity = engine.create_entity()
            
            -- Test texture loading
            local texture = engine.load_texture("test.png")
            
            -- Test logging
            engine.log("info", "API test successful!")
            
            return "API test passed! Max entities: " .. max_entities .. ", Entity: " .. tostring(entity) .. ", Texture: " .. tostring(texture)
        end
        
        function test_v2_arrays()
            -- Test v2 flat array format (stride=6 for transforms)
            local transforms = { 1, 10.0, 20.0, 0.0, 1.0, 1.0 }  -- id, x, y, rot, sx, sy
            engine.set_transforms(transforms)
            
            -- Test v2 flat array format (stride=10 for sprites)  
            local entity = engine.create_entity()
            local texture = engine.load_texture("sprite.png")
            local sprites = { entity, texture, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0 }
            engine.submit_sprites(sprites)
            
            return "v2 array format test passed!"
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
    
    sandbox.load_script(test_script, "api_test.lua")?;
    
    // Test API functionality
    let api_result: String = sandbox.call_function("test_api", ())?;
    info!("API test result: {}", api_result);
    
    // Test v2 array format
    let array_result: String = sandbox.call_function("test_v2_arrays", ())?;
    info!("Array test result: {}", array_result);
    
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
    info!("Engine API and sandbox test completed");
    
    Ok(())
}