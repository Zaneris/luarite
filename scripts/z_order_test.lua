-- Z-ordering test: Add sprites in WRONG order but with CORRECT z-values
-- This test verifies that the renderer correctly sorts by z-value regardless of submission order

assert(engine.api_version == 1)

local w, h = 320, 180

-- Create entities in wrong order (we'll add back sprite first, front sprite last)
local front_entity = engine.create_entity()  -- Will be z=2.0 (should appear on top)
local middle_entity = engine.create_entity() -- Will be z=1.0 (should appear in middle)
local back_entity = engine.create_entity()   -- Will be z=0.0 (should appear behind)

-- Create a dummy texture (will fail to load but that's OK for testing)
local tex = engine.load_texture("dummy.png")

-- Create buffers
local T = engine.create_transform_buffer(3)
local S = engine.create_sprite_buffer(3)

function on_start()
  engine.set_clear_color(0.0, 0.0, 0.0, 1.0) -- Black background
  engine.set_render_resolution("retro")
  
  -- INTENTIONALLY ADD SPRITES IN WRONG ORDER
  -- Add front sprite first (index 1) but give it highest z-value
  S:set_tex(1, front_entity, tex)
  S:set_color(1, 0.0, 0.0, 1.0, 1.0)  -- Pure blue - should appear ON TOP
  S:set_uv_rect(1, 0.0, 0.0, 1.0, 1.0)
  S:set_z(1, 2.0)  -- HIGHEST z-value = front layer
  
  -- Add middle sprite second (index 2) 
  S:set_tex(2, middle_entity, tex)
  S:set_color(2, 0.0, 1.0, 0.0, 1.0)  -- Pure green - should appear IN MIDDLE
  S:set_uv_rect(2, 0.0, 0.0, 1.0, 1.0)
  S:set_z(2, 1.0)  -- MIDDLE z-value = middle layer
  
  -- Add back sprite last (index 3) but give it lowest z-value
  S:set_tex(3, back_entity, tex)
  S:set_color(3, 1.0, 0.0, 0.0, 1.0)  -- Pure red - should appear BEHIND
  S:set_uv_rect(3, 0.0, 0.0, 1.0, 1.0)
  S:set_z(3, 0.0)  -- LOWEST z-value = back layer
end

function on_update(dt)
  -- Position all sprites to overlap at center - this is critical for testing z-ordering
  local center_x, center_y = w * 0.5, h * 0.5
  local size = 80
  
  -- All sprites positioned at same location to overlap completely
  -- The submission order is: FRONT (blue), MIDDLE (green), BACK (red)
  -- The z-order should make it render as: BACK (red) behind, MIDDLE (green) middle, FRONT (blue) on top
  -- So at center pixel we should see BLUE color
  
  T:set(1, front_entity, center_x, center_y, 0, size, size)  -- Blue sprite (z=2.0, should be on top)
  T:set(2, middle_entity, center_x, center_y, 0, size, size) -- Green sprite (z=1.0, should be middle)  
  T:set(3, back_entity, center_x, center_y, 0, size, size)   -- Red sprite (z=0.0, should be behind)
  
  -- Commit - this will submit sprites in the wrong order but with correct z-values
  engine.set_transforms(T)
  engine.submit_sprites(S)
end