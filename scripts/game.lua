-- Pong demo using sugar drawing API

assert(engine.api_version == 1)

local K = engine.keys

-- Deterministic RNG for replay
engine.seed(1337)

-- Virtual canvas coordinates (320x180)
local w, h = 320, 180
local PADDLE_W, PADDLE_H = 10, 48
local BALL_SIZE          = 8
local SPEED_PADDLE       = 140.0
local BASE_VX, BASE_VY   = 120.0, 90.0

-- Entities/handles/state
local background, paddle_l, paddle_r, ball, tex, atlas
local px_l, py_l = 40.0, 0.0
local px_r, py_r = 0.0, 0.0
local bx, by     = 0.0, 0.0
local vx, vy     = BASE_VX, BASE_VY
local score_l, score_r = 0, 0
local hud_t = 0.0

-- Colors using helper functions
local bg_color = engine.rgba(26, 26, 26, 255)      -- Dark gray background
local paddle_l_color = engine.rgba(51, 204, 51, 255)  -- Green left paddle  
local paddle_r_color = engine.rgba(51, 51, 204, 255)  -- Blue right paddle
local ball_color = engine.rgba(230, 230, 51, 255)     -- Yellow ball

local function reset_ball()
  bx, by = w*0.5, h*0.5
  local dirx = engine.random_bool() and -1 or 1
  local diry = engine.random_bool() and -1 or 1
  vx, vy = dirx*BASE_VX, diry*BASE_VY
end

local function aabb_hit(cx, cy, hw, hh, x, y, s)
  local ax0, ay0, ax1, ay1 = cx-hw, cy-hh, cx+hw, cy+hh
  local bx0, by0, bx1, by1 = x-s,  y-s,   x+s,   y+s
  return not (ax1 < bx0 or bx1 < ax0 or ay1 < by0 or by1 < ay0)
end

local function axis(inp, posKey, negKey)
  local p = inp:down(posKey) and 1 or 0
  local n = inp:down(negKey) and -1 or 0
  return p + n
end

function on_start()
  -- Explicitly set background to black and use retro virtual resolution (320x180)
  engine.set_clear_color(0.0, 0.0, 0.0)
  engine.set_render_mode("retro")
  background = engine.create_entity()
  paddle_l = engine.create_entity()
  paddle_r = engine.create_entity()
  ball     = engine.create_entity()

  atlas = engine.atlas_load("assets/atlas.png", "assets/atlas.json")
  tex   = atlas and atlas:tex() or engine.load_texture("assets/atlas.png")

  px_l, py_l = 20.0, h*0.5
  px_r, py_r = w-20.0, h*0.5
  reset_ball()
end

function on_update(dt)
  local inp = engine.get_input()
  local dyL = axis(inp, K.KeyW, K.KeyS)
  local dyR = axis(inp, K.ArrowUp, K.ArrowDown)

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

  -- Draw using sugar API
  engine.begin_frame()
  
  -- Background
  engine.sprite{
    entity = background,
    texture = tex,
    pos = {w*0.5, h*0.5},
    size = {w, h},
    color = bg_color,
    uv = {0, 0, 1, 1},
    z = -1
  }
  
  -- Left paddle (green)
  if atlas then
    engine.sprite{
      entity = paddle_l,
      atlas = {ref = atlas, name = "paddle"},
      pos = {px_l, py_l},
      size = {PADDLE_W, PADDLE_H},
      color = paddle_l_color,
      z = 0
    }
  else
    engine.sprite{
      entity = paddle_l,
      texture = tex,
      pos = {px_l, py_l},
      size = {PADDLE_W, PADDLE_H},
      color = paddle_l_color,
      uv = {0, 0, 1, 1},
      z = 0
    }
  end
  
  -- Right paddle (blue)
  if atlas then
    engine.sprite{
      entity = paddle_r,
      atlas = {ref = atlas, name = "paddle"},
      pos = {px_r, py_r},
      size = {PADDLE_W, PADDLE_H},
      color = paddle_r_color,
      z = 0
    }
  else
    engine.sprite{
      entity = paddle_r,
      texture = tex,
      pos = {px_r, py_r},
      size = {PADDLE_W, PADDLE_H},
      color = paddle_r_color,
      uv = {0, 0, 1, 1},
      z = 0
    }
  end
  
  -- Ball (yellow)
  if atlas then
    engine.sprite{
      entity = ball,
      atlas = {ref = atlas, name = "ball"},
      pos = {bx, by},
      size = BALL_SIZE,
      color = ball_color,
      z = 1
    }
  else
    engine.sprite{
      entity = ball,
      texture = tex,
      pos = {bx, by},
      size = BALL_SIZE,
      color = ball_color,
      uv = {0, 0, 1, 1},
      z = 1
    }
  end
  
  engine.end_frame()

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
