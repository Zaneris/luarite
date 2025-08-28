-- Example Luarite script demonstrating the v2 array API
-- This script creates a simple animated sprite

-- Assert API version compatibility
assert(engine.api_version == 1, "Script requires API version 1, got " .. tostring(engine.api_version))

-- Reusable arrays to avoid GC pressure (per performance guidelines)
local transforms, sprites = {}, {}
local entity, texture, time = nil, nil, 0
local hud_t = 0

-- v2 array helpers (local module-style table)
local v2 = {}
function v2.set_transform(arr, idx, id, x, y, rot, sx, sy)
  local o = (idx-1)*6
  arr[o+1], arr[o+2], arr[o+3] = id, x, y
  arr[o+4], arr[o+5], arr[o+6] = rot, sx, sy
end
function v2.set_sprite(arr, idx, id, tex, u0, v0, u1, v1, r, g, b, a)
  local o = (idx-1)*10
  arr[o+1], arr[o+2] = id, tex
  arr[o+3], arr[o+4], arr[o+5], arr[o+6] = u0, v0, u1, v1
  arr[o+7], arr[o+8], arr[o+9], arr[o+10] = r, g, b, a
end

function on_start()
    -- Try to restore persistent state or create new entity
    entity = engine.restore("player_entity") or engine.create_entity()
    engine.persist("player_entity", entity)

    -- Load texture atlas
    texture = engine.load_texture("assets/atlas.png")
    
    -- Initialize transform array (v2 format: stride=6)
    -- Format: id, x, y, rotation, scale_x, scale_y
    v2.set_transform(transforms, 1, entity, 0.0, 0.0, 0.0, 1.0, 1.0)
    
    -- Initialize sprite array (v2 format: stride=10) 
    -- Format: id, texture, u0, v0, u1, v1, r, g, b, a
    v2.set_sprite(sprites, 1, entity, texture, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0)
    
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
