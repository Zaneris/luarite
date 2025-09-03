-- Z-ordering test with partial overlap: Tests more complex z-ordering scenarios
-- Add sprites in wrong order with partial overlaps to test specific pixel regions

assert(engine.api_version == 1)

local w, h = 320, 180

-- Create entities  
local sprite_a = engine.create_entity()  -- Will be z=3.0 (highest, should be on top)
local sprite_b = engine.create_entity()  -- Will be z=1.0 (lowest, should be behind) 
local sprite_c = engine.create_entity()  -- Will be z=2.0 (middle)

local tex = engine.load_texture("dummy.png")
local T = engine.create_transform_buffer(3)
local S = engine.create_sprite_buffer(3)

function on_start()
  engine.set_clear_color(0.0, 0.0, 0.0, 1.0) -- Black background
  engine.set_render_mode("retro")
  
  -- Add in WRONG submission order: B first, A second, C last
  -- But z-values will determine render order: B(z=1.0) behind, C(z=2.0) middle, A(z=3.0) front
  
  -- Submit sprite B FIRST (index 1) but it should render BEHIND (lowest z)
  S:set_tex(1, sprite_b, tex)
  S:set_color(1, 1.0, 0.0, 0.0, 1.0)  -- RED - should be behind everything
  S:set_uv_rect(1, 0.0, 0.0, 1.0, 1.0)
  S:set_z(1, 1.0)  -- LOWEST z-value
  
  -- Submit sprite A SECOND (index 2) but it should render ON TOP (highest z)  
  S:set_tex(2, sprite_a, tex)
  S:set_color(2, 0.0, 0.0, 1.0, 1.0)  -- BLUE - should be on top
  S:set_uv_rect(2, 0.0, 0.0, 1.0, 1.0)
  S:set_z(2, 3.0)  -- HIGHEST z-value
  
  -- Submit sprite C LAST (index 3) but it should render in MIDDLE (middle z)
  S:set_tex(3, sprite_c, tex)  
  S:set_color(3, 0.0, 1.0, 0.0, 1.0)  -- GREEN - should be in middle
  S:set_uv_rect(3, 0.0, 0.0, 1.0, 1.0)
  S:set_z(3, 2.0)  -- MIDDLE z-value
end

function on_update(dt)
  -- Create partial overlaps so we can test specific regions
  -- Layout:    [RED]
  --          [GREEN]
  --            [BLUE]
  --
  -- In overlap areas:
  -- - RED only area should show red
  -- - GREEN only area should show green  
  -- - BLUE only area should show blue
  -- - RED+GREEN overlap should show GREEN (green z=2.0 > red z=1.0)
  -- - GREEN+BLUE overlap should show BLUE (blue z=3.0 > green z=2.0) 
  -- - All three overlap should show BLUE (blue z=3.0 is highest)
  
  local size = 60
  
  -- Position sprites to create predictable overlaps
  T:set(1, sprite_b, 140, 90, 0, size, size)   -- Red (z=1.0) - leftmost
  T:set(2, sprite_a, 180, 90, 0, size, size)   -- Blue (z=3.0) - rightmost  
  T:set(3, sprite_c, 160, 90, 0, size, size)   -- Green (z=2.0) - center
  
  engine.set_transforms(T)
  engine.submit_sprites(S)
end