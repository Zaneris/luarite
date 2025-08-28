-- Example Luarite script demonstrating the v2 array API
-- This script creates a simple animated sprite

-- Assert API version compatibility
assert(engine.api_version == 1, "Script requires API version 1, got " .. tostring(engine.api_version))

-- Reusable arrays to avoid GC pressure (per performance guidelines)
local transforms, sprites = {}, {}
local entity, texture, time = nil, nil, 0
local hud_t = 0

function on_start()
    -- Try to restore persistent state or create new entity
    entity = engine.restore("player_entity") or engine.create_entity()
    engine.persist("player_entity", entity)

    -- Load texture atlas
    texture = engine.load_texture("assets/atlas.png")
    
    -- Initialize transform array (v2 format: stride=6)
    -- Format: id, x, y, rotation, scale_x, scale_y
    transforms[1], transforms[2], transforms[3] = entity, 0.0, 0.0  -- id, x, y
    transforms[4], transforms[5], transforms[6] = 0.0, 1.0, 1.0    -- rot, sx, sy
    
    -- Initialize sprite array (v2 format: stride=10) 
    -- Format: id, texture, u0, v0, u1, v1, r, g, b, a
    sprites[1], sprites[2] = entity, texture                       -- id, tex
    sprites[3], sprites[4], sprites[5], sprites[6] = 0.0, 0.0, 1.0, 1.0  -- u0, v0, u1, v1 (full texture)
    sprites[7], sprites[8], sprites[9], sprites[10] = 1.0, 1.0, 1.0, 1.0 -- r, g, b, a (white)
    
    engine.log("info", "Game script initialized with entity: " .. tostring(entity))
end

function on_update(dt)
    -- Update time and animate position
    time = time + dt
    hud_t = hud_t + dt
    
    -- Simple sine wave animation
    transforms[2] = math.sin(time) * 100.0  -- x position
    transforms[3] = math.cos(time) * 50.0   -- y position
    transforms[4] = time * 0.5              -- rotation
    
    -- Submit batched updates to engine
    engine.set_transforms(transforms)
    engine.submit_sprites(sprites)

    -- Minimal HUD logger: once per second
    if hud_t >= 1.0 then
        hud_t = 0
        local m = engine.get_metrics()
        engine.log("info", string.format(
            "HUD cpu=%.2fms sprites=%d ffi=%d",
            m.cpu_frame_ms, m.sprites_submitted, m.ffi_calls
        ))
    end
end

function on_reload(old_env)
    -- Migrate state during hot reload
    if old_env then
        time = old_env.time or time
        engine.log("info", "State migrated during reload, time: " .. tostring(time))
    end
end
