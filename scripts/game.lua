-- Pong demo using typed buffers (QoL)

assert(engine.api_version == 1)

-- Deterministic RNG for replay
if engine.seed then engine.seed(1337) else math.randomseed(1337) end

-- Pixels → units
engine.units.set_pixels_per_unit(64)

-- Window & constants
local w, h = 1024, 768
local PADDLE_W, PADDLE_H = 16, 100
local BALL_SIZE          = 16
local SPEED_PADDLE       = 420.0
local BASE_VX, BASE_VY   = 280.0, 180.0

-- Entities/handles/state
local paddle_l, paddle_r, ball, tex, atlas
local px_l, py_l = 40.0, 0.0
local px_r, py_r = 0.0, 0.0
local bx, by     = 0.0, 0.0
local vx, vy     = BASE_VX, BASE_VY
local score_l, score_r = 0, 0
local hud_t = 0.0

-- Typed buffers (capacity 3)
local T = engine.create_transform_buffer(3)
local S = engine.create_sprite_buffer(3)
local fb

local function reset_ball()
  bx, by = w*0.5, h*0.5
  local dirx = (math.random() < 0.5) and -1 or 1
  local diry = (math.random() < 0.5) and -1 or 1
  vx, vy = dirx*BASE_VX, diry*BASE_VY
end

local function aabb_hit(cx, cy, hw, hh, x, y, s)
  local ax0, ay0, ax1, ay1 = cx-hw, cy-hh, cx+hw, cy+hh
  local bx0, by0, bx1, by1 = x-s,  y-s,   x+s,   y+s
  return not (ax1 < bx0 or bx1 < ax0 or ay1 < by0 or by1 < ay0)
end

local function axis(inp, posKey, negKey)
  local p = inp:get_key(posKey) and 1 or 0
  local n = inp:get_key(negKey) and -1 or 0
  return p + n
end

function on_start()
  local ww, hh = engine.window_size(); if ww and hh then w, h = ww, hh end
  paddle_l = engine.create_entity()
  paddle_r = engine.create_entity()
  ball     = engine.create_entity()

  atlas = engine.atlas_load("assets/atlas.png", "assets/atlas.json")
  tex   = atlas and atlas:tex() or engine.load_texture("assets/atlas.png")

  px_l, py_l = 40.0, h*0.5
  px_r, py_r = w-40.0, h*0.5
  reset_ball()

  -- Static sprite attributes set once
  S:set_tex(1, paddle_l, tex); S:set_color(1, 0.2,0.8,0.2,1.0); if atlas then S:set_named_uv(1, atlas, "paddle") else S:set_uv_rect(1, 0.0,0.0,1.0,1.0) end
  S:set_tex(2, paddle_r, tex); S:set_color(2, 0.2,0.2,0.8,1.0); if atlas then S:set_named_uv(2, atlas, "paddle") else S:set_uv_rect(2, 0.0,0.0,1.0,1.0) end
  S:set_tex(3, ball,     tex); S:set_color(3, 0.9,0.9,0.2,1.0); if atlas then S:set_named_uv(3, atlas, "ball")   else S:set_uv_rect(3, 0.0,0.0,1.0,1.0) end
  fb = engine.frame_builder(T, S)
end

function on_update(dt)
  local inp = engine.get_input()
  local dyL = axis(inp, "KeyW", "KeyS")
  local dyR = axis(inp, "ArrowUp", "ArrowDown")

  local phh = PADDLE_H*0.5
  py_l = math.min(math.max(py_l + dyL*SPEED_PADDLE*dt, phh), h - phh)
  py_r = math.min(math.max(py_r + dyR*SPEED_PADDLE*dt, phh), h - phh)

  bx = bx + vx*dt; by = by + vy*dt
  local bh = BALL_SIZE*0.5
  if by < bh then by = bh; vy = -vy end
  if by > h - bh then by = h - bh; vy = -vy end

  if aabb_hit(px_l, py_l, PADDLE_W*0.5, PADDLE_H*0.5, bx, by, bh) and vx < 0 then
    vx = -vx; bx = px_l + PADDLE_W*0.5 + bh + 1
  end
  if aabb_hit(px_r, py_r, PADDLE_W*0.5, PADDLE_H*0.5, bx, by, bh) and vx > 0 then
    vx = -vx; bx = px_r - PADDLE_W*0.5 - bh - 1
  end

  if bx < -20 then score_r = score_r + 1; reset_ball()
  elseif bx > w + 20 then score_l = score_l + 1; reset_ball() end

  -- Fill transforms via builder and commit once
  fb:transform_px(1, paddle_l, px_l, py_l, 0, PADDLE_W, PADDLE_H)
  fb:transform_px(2, paddle_r, px_r, py_r, 0, PADDLE_W, PADDLE_H)
  fb:transform_px(3, ball,     bx,   by,   0, BALL_SIZE, BALL_SIZE)
  fb:commit()

  hud_t = hud_t + dt
  if hud_t >= 1.0 then
    hud_t = 0.0
    local m = engine.get_metrics()
    engine.hud_printf(string.format("L:%d R:%d | cpu=%.2fms | sprites=%d", score_l, score_r, m.cpu_frame_ms, m.sprites_submitted))
  end
end

function on_reload(old_env)
  if old_env then end
end
