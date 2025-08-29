-- Pong-like demo using v2 array API

assert(engine.api_version == 1, "Script requires API version 1, got " .. tostring(engine.api_version))

-- Reusable arrays
local transforms, sprites = {}, {}

-- Entities and texture handle
local paddle_l, paddle_r, ball, tex

-- State
local w, h = 1024, 768
local paddle_w, paddle_h = 16, 100
local ball_size = 16
local speed_paddle = 420.0
local vx, vy = 280.0, 180.0 -- ball velocity in px/s
local px_l, py_l = 40.0, 0.0
local px_r, py_r = 0.0, 0.0
local bx, by = 0.0, 0.0
local score_l, score_r = 0, 0
local hud_t = 0

-- v2 helpers
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

local function reset_ball()
  bx, by = w * 0.5, h * 0.5
  local dir = (math.random() < 0.5) and -1.0 or 1.0
  vx = dir * 280.0
  vy = (math.random() * 2.0 - 1.0) * 220.0
end

function on_start()
  -- Window size
  local ww, hh = engine.window_size()
  if ww and hh then w, h = ww, hh end

  -- Entities
  paddle_l = engine.create_entity()
  paddle_r = engine.create_entity()
  ball     = engine.create_entity()
  -- Texture (will fall back to white if missing)
  tex = engine.load_texture("assets/atlas.png")

  -- Initial positions
  px_l, py_l = 40.0, h * 0.5
  px_r, py_r = w - 40.0, h * 0.5
  reset_ball()

  -- Pre-size arrays for 3 entities
  v2.set_transform(transforms, 1, paddle_l, px_l, py_l, 0.0, paddle_w/64.0, paddle_h/64.0)
  v2.set_transform(transforms, 2, paddle_r, px_r, py_r, 0.0, paddle_w/64.0, paddle_h/64.0)
  v2.set_transform(transforms, 3, ball,     bx,   by,   0.0, ball_size/64.0, ball_size/64.0)

  v2.set_sprite(sprites, 1, paddle_l, tex, 0.0, 0.0, 1.0, 1.0, 0.2, 0.8, 0.2, 1.0)
  v2.set_sprite(sprites, 2, paddle_r, tex, 0.0, 0.0, 1.0, 1.0, 0.2, 0.2, 0.8, 1.0)
  v2.set_sprite(sprites, 3, ball,     tex, 0.0, 0.0, 1.0, 1.0, 0.9, 0.9, 0.2, 1.0)

  engine.log("info", string.format("Pong start %dx%d", w, h))
end

local function aabb_hit(cx, cy, hw, hh, x, y, s)
  -- AABB overlap: center/hw/hh for paddle, center/half-size for ball
  local ax0, ay0 = cx - hw, cy - hh
  local ax1, ay1 = cx + hw, cy + hh
  local bx0, by0 = x - s,  y - s
  local bx1, by1 = x + s,  y + s
  return not (ax1 < bx0 or bx1 < ax0 or ay1 < by0 or by1 < ay0)
end

function on_update(dt)
  -- Input
  local inp = engine.get_input()
  local upL   = inp:get_key("KeyW")
  local downL = inp:get_key("KeyS")
  local upR   = inp:get_key("ArrowUp")
  local downR = inp:get_key("ArrowDown")

  -- With current projection (Y up), increasing Y moves up on screen
  if upL   then py_l = py_l + speed_paddle * dt end
  if downL then py_l = py_l - speed_paddle * dt end
  if upR   then py_r = py_r + speed_paddle * dt end
  if downR then py_r = py_r - speed_paddle * dt end

  -- Clamp paddles
  local phh = paddle_h * 0.5
  if py_l < phh then py_l = phh end
  if py_l > h - phh then py_l = h - phh end
  if py_r < phh then py_r = phh end
  if py_r > h - phh then py_r = h - phh end

  -- Move ball
  bx = bx + vx * dt
  by = by + vy * dt

  -- Wall bounce (top/bottom)
  local bh = ball_size * 0.5
  if by < bh then by = bh; vy = -vy end
  if by > h - bh then by = h - bh; vy = -vy end

  -- Paddle collisions
  if aabb_hit(px_l, py_l, paddle_w*0.5, paddle_h*0.5, bx, by, bh) and vx < 0.0 then
    vx = -vx
    bx = px_l + paddle_w*0.5 + bh + 1.0
  end
  if aabb_hit(px_r, py_r, paddle_w*0.5, paddle_h*0.5, bx, by, bh) and vx > 0.0 then
    vx = -vx
    bx = px_r - paddle_w*0.5 - bh - 1.0
  end

  -- Scoring (left/right walls)
  if bx < -20.0 then
    score_r = score_r + 1
    engine.log("info", string.format("Score L:%d R:%d", score_l, score_r))
    reset_ball()
  elseif bx > w + 20.0 then
    score_l = score_l + 1
    engine.log("info", string.format("Score L:%d R:%d", score_l, score_r))
    reset_ball()
  end

  -- Update arrays
  v2.set_transform(transforms, 1, paddle_l, px_l, py_l, 0.0, paddle_w/64.0, paddle_h/64.0)
  v2.set_transform(transforms, 2, paddle_r, px_r, py_r, 0.0, paddle_w/64.0, paddle_h/64.0)
  v2.set_transform(transforms, 3, ball,     bx,   by,   0.0, ball_size/64.0, ball_size/64.0)

  engine.set_transforms(transforms)
  engine.submit_sprites(sprites)

  -- HUD log each second
  hud_t = hud_t + dt
  if hud_t >= 1.0 then
    hud_t = 0.0
    local m = engine.get_metrics()
    engine.log("info", string.format("cpu=%.2fms sprites=%d ffi=%d", m.cpu_frame_ms, m.sprites_submitted, m.ffi_calls))
  end
end

function on_reload(old_env)
    -- Migrate state during hot reload
    if old_env then
        time = old_env.time or time
        engine.log("info", "State migrated during reload, time: " .. tostring(time))
    end
end
