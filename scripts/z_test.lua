-- Z-depth test demo
-- This demonstrates that z-values work correctly in the Lua scripting layer

assert(engine.api_version == 1)

local w, h = 320, 180

-- Create entities
local back_entity = engine.create_entity()
local mid_entity = engine.create_entity()
local front_entity = engine.create_entity()
local tex = engine.load_texture("assets/atlas.png")

-- Create buffers
local T = engine.create_transform_buffer(3)
local S = engine.create_sprite_buffer(3)

function on_start()
  engine.set_clear_color(0.0, 0.0, 0.0)
  engine.set_render_mode("retro")
  
  -- Set up sprites with different z-values
  -- Back sprite (red, z=0.0) - should appear behind
  S:set_tex(1, back_entity, tex)
  S:set_color(1, 1.0, 0.0, 0.0, 0.8)  -- Red with alpha
  S:set_uv_rect(1, 0.0, 0.0, 1.0, 1.0)
  S:set_z(1, 0.0)  -- Back layer
  
  -- Front sprite (blue, z=2.0) - should appear in front
  S:set_tex(2, front_entity, tex)
  S:set_color(2, 0.0, 0.0, 1.0, 0.8)  -- Blue with alpha
  S:set_uv_rect(2, 0.0, 0.0, 1.0, 1.0)
  S:set_z(2, 2.0)  -- Front layer
  
  -- Middle sprite (green, z=1.0) - should appear in middle
  S:set_tex(3, mid_entity, tex)
  S:set_color(3, 0.0, 1.0, 0.0, 0.8)  -- Green with alpha
  S:set_uv_rect(3, 0.0, 0.0, 1.0, 1.0)
  S:set_z(3, 1.0)  -- Middle layer
end

function on_update(dt)
  local time = engine.time()
  
  -- Position sprites so they overlap and demonstrate z-ordering
  local offset = math.sin(time) * 20
  
  -- All sprites positioned to overlap in center area
  T:set(1, back_entity, w*0.5 - 10, h*0.5 + offset, 0, 60, 60)       -- Back (red)
  T:set(2, front_entity, w*0.5 + 10, h*0.5 - offset, 0, 60, 60)      -- Front (blue)  
  T:set(3, mid_entity, w*0.5, h*0.5, 0, 60, 60)                      -- Middle (green)
  
  -- Commit transforms and sprites
  engine.set_transforms(T)
  engine.submit_sprites(S)
  
  -- Log z-values for debugging
  if math.floor(time * 2) % 4 == 0 then
    engine.log("info", "Z-test running - Red(z=0.0) should be behind, Green(z=1.0) in middle, Blue(z=2.0) in front")
  end
end