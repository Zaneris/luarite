use anyhow::Result;
use mlua::{AnyUserData, FromLua, Lua, UserData, UserDataMethods, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use serde::Deserialize;

/// Current engine API version
pub const API_VERSION: u32 = 1;

/// Engine handle types (opaque to Lua scripts)
#[derive(Debug, Clone, Copy)]
pub struct EntityId(pub u32);

#[derive(Debug, Clone, Copy)]
pub struct TextureHandle(pub u32);

impl UserData for EntityId {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, this, ()| {
            Ok(format!("Entity({})", this.0))
        });
    }
}

impl UserData for TextureHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, this, ()| {
            Ok(format!("Texture({})", this.0))
        });
    }
}

/// Input snapshot structure for deterministic input
#[derive(Debug, Clone)]
pub struct InputSnapshot {
    pub keys: HashMap<String, bool>,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub mouse_buttons: HashMap<String, bool>,
}

impl InputSnapshot {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_buttons: HashMap::new(),
        }
    }
}

impl Default for InputSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

impl UserData for InputSnapshot {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_key", |_, this, key: String| {
            Ok(this.keys.get(&key).copied().unwrap_or(false))
        });

        methods.add_method("get_mouse_button", |_, this, button: String| {
            Ok(this.mouse_buttons.get(&button).copied().unwrap_or(false))
        });

        methods.add_method("mouse_pos", |_, this, ()| Ok((this.mouse_x, this.mouse_y)));
    }
}

/// Engine capabilities for API handshake
#[derive(Debug)]
pub struct EngineCapabilities {
    pub max_entities: u32,
    pub max_textures: u32,
    pub supports_hot_reload: bool,
    pub supports_persistence: bool,
    pub v2_arrays: bool,
    pub v3_packed_blobs: bool,
}

impl Default for EngineCapabilities {
    fn default() -> Self {
        Self {
            max_entities: 10000,
            max_textures: 1000,
            supports_hot_reload: true,
            supports_persistence: true,
            v2_arrays: true,
            v3_packed_blobs: false, // Will implement later
        }
    }
}

impl UserData for EngineCapabilities {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("max_entities", |_, this, ()| Ok(this.max_entities));
        methods.add_method("max_textures", |_, this, ()| Ok(this.max_textures));
        methods.add_method("supports_hot_reload", |_, this, ()| {
            Ok(this.supports_hot_reload)
        });
        methods.add_method("supports_persistence", |_, this, ()| {
            Ok(this.supports_persistence)
        });
        methods.add_method("v2_arrays", |_, this, ()| Ok(this.v2_arrays));
        methods.add_method("v3_packed_blobs", |_, this, ()| Ok(this.v3_packed_blobs));
    }
}

/// Main engine API struct
pub struct EngineApi {
    next_entity_id: u32,
    next_texture_id: u32,
    fixed_time: Rc<RefCell<f64>>, // shared with time() closure
    persistence_store: Rc<RefCell<HashMap<String, Value>>>,
    rng_state: Rc<RefCell<u64>>, // deterministic RNG
    pixels_per_unit: Rc<RefCell<f64>>, // units helper
    #[allow(dead_code)] // TODO: will be used for capability queries  
    capabilities: EngineCapabilities,
}

impl EngineApi {
    pub fn new() -> Self {
        Self {
            next_entity_id: 1,
            next_texture_id: 1,
            fixed_time: Rc::new(RefCell::new(0.0)),
            persistence_store: Rc::new(RefCell::new(HashMap::new())),
            rng_state: Rc::new(RefCell::new(0x9E3779B97F4A7C15)),
            pixels_per_unit: Rc::new(RefCell::new(64.0)),
            capabilities: EngineCapabilities::default(), // TODO: will be used for capability queries
        }
    }

    pub fn update_time(&mut self, dt: f64) {
        *self.fixed_time.borrow_mut() += dt;
    }

    pub fn setup_engine_namespace(&self, lua: &Lua) -> Result<()> {
        let globals = lua.globals();

        // Create main engine table
        let engine_table = match lua.create_table() {
            Ok(table) => table,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create engine table: {}",
                    e
                )))
            }
        };

        // Constructors: typed buffers
        {
            let ppu = self.pixels_per_unit.clone();
            let tb_ctor = lua
                .create_function(move |_, cap: usize| Ok(TransformBuffer::new(cap, ppu.clone())))
                .map_err(|e| anyhow::Error::msg(format!("Failed to create create_transform_buffer: {}", e)))?;
            engine_table
                .set("create_transform_buffer", tb_ctor)
                .map_err(|e| anyhow::Error::msg(format!("Failed to set create_transform_buffer: {}", e)))?;

            let sb_ctor = lua
                .create_function(move |_, cap: usize| Ok(SpriteBuffer::new(cap)))
                .map_err(|e| anyhow::Error::msg(format!("Failed to create create_sprite_buffer: {}", e)))?;
            engine_table
                .set("create_sprite_buffer", sb_ctor)
                .map_err(|e| anyhow::Error::msg(format!("Failed to set create_sprite_buffer: {}", e)))?;
        }

        // API version and capabilities
        if let Err(e) = engine_table.set("api_version", API_VERSION) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set api_version: {}",
                e
            )));
        }

        // Set up get_capabilities function
        let caps_func = match lua.create_function(move |_, ()| Ok(EngineCapabilities::default())) {
            Ok(func) => func,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create get_capabilities: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("get_capabilities", caps_func) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set get_capabilities: {}",
                e
            )));
        }

        // Entity management
        let next_entity_id = std::cell::RefCell::new(self.next_entity_id);
        let entity_func = match lua.create_function(move |_, ()| {
            let mut id = next_entity_id.borrow_mut();
            let entity = EntityId(*id);
            *id += 1;
            Ok(entity)
        }) {
            Ok(func) => func,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create create_entity: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("create_entity", entity_func) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set create_entity: {}",
                e
            )));
        }

        // Texture loading
        let next_texture_id = std::cell::RefCell::new(self.next_texture_id);
        let texture_func = match lua.create_function(move |_, path: String| {
            tracing::info!("Loading texture: {}", path);
            let mut id = next_texture_id.borrow_mut();
            let texture = TextureHandle(*id);
            *id += 1;
            Ok(texture)
        }) {
            Ok(func) => func,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create load_texture: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("load_texture", texture_func) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set load_texture: {}",
                e
            )));
        }

        // Transform batching (accept v2 array or TransformBuffer)
        let transform_func = match lua.create_function(|lua, v: Value| {
            match v {
                Value::UserData(ud) => {
                    if let Ok(tb) = ud.borrow::<TransformBuffer>() {
                        let rows = *tb.len.borrow();
                        let buf = tb.buf.borrow();
                        if rows * 6 > buf.len() {
                            return Err(mlua::Error::RuntimeError("TransformBuffer length exceeds capacity".into()));
                        }
                        tracing::debug!("Setting {} transforms (typed)", rows);
                        Ok(())
                    } else {
                        Err(mlua::Error::RuntimeError("ARG_ERROR: unsupported userdata for set_transforms".into()))
                    }
                }
                Value::Table(arr) => {
                    let len = arr.raw_len();
                    if len % 6 != 0 {
                        return Err(mlua::Error::RuntimeError(format!(
                            "ARG_ERROR: set_transforms stride mismatch (got={}, want=6)",
                            len % 6
                        )));
                    }
                    tracing::debug!("Setting {} transforms", len / 6);
                    Ok(())
                }
                _ => Err(mlua::Error::RuntimeError(
                    "ARG_ERROR: set_transforms expects table or TransformBuffer".into(),
                )),
            }
        }) {
            Ok(func) => func,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create set_transforms: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("set_transforms", transform_func) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set set_transforms: {}",
                e
            )));
        }

        // Sprite batching (accept v2 array or SpriteBuffer)
        let sprite_func = match lua.create_function(|lua, v: Value| {
            match v {
                Value::UserData(ud) => {
                    if let Ok(_sb) = ud.borrow::<SpriteBuffer>() {
                        tracing::debug!("Submitting sprites (typed)");
                        Ok(())
                    } else {
                        Err(mlua::Error::RuntimeError("ARG_ERROR: unsupported userdata for submit_sprites".into()))
                    }
                }
                Value::Table(t) => {
                    let len = t.raw_len();
                    if len % 10 != 0 {
                        return Err(mlua::Error::RuntimeError(format!(
                            "ARG_ERROR: submit_sprites stride mismatch (got={}, want=10)",
                            len % 10
                        )));
                    }
                    tracing::debug!("Submitting {} sprites", len / 10);
                    Ok(())
                }
                _ => Err(mlua::Error::RuntimeError(
                    "ARG_ERROR: submit_sprites expects table or SpriteBuffer".into(),
                )),
            }
        }) {
            Ok(func) => func,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create submit_sprites: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("submit_sprites", sprite_func) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set submit_sprites: {}",
                e
            )));
        }

        // Input system
        let input_func = match lua.create_function(|_, ()| Ok(InputSnapshot::new())) {
            Ok(func) => func,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create get_input: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("get_input", input_func) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set get_input: {}",
                e
            )));
        }

        // Time system
        let fixed_time = self.fixed_time.clone();
        let time_func = match lua.create_function(move |_, ()| Ok(*fixed_time.borrow())) {
            Ok(func) => func,
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create time: {}", e))),
        };
        if let Err(e) = engine_table.set("time", time_func) {
            return Err(anyhow::Error::msg(format!("Failed to set time: {}", e)));
        }

        // Units helper (pixels per unit)
        let ppu_ref = self.pixels_per_unit.clone();
        let set_ppu = lua
            .create_function(move |_, n: f64| {
                if n <= 0.0 {
                    return Err(mlua::Error::RuntimeError(
                        "pixels_per_unit must be > 0".into(),
                    ));
                }
                *ppu_ref.borrow_mut() = n;
                Ok(())
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to create set_pixels_per_unit: {}", e)))?;
        let units_tbl = lua
            .create_table()
            .map_err(|e| anyhow::Error::msg(format!("Failed to create units table: {}", e)))?;
        units_tbl
            .set("set_pixels_per_unit", set_ppu)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set units fn: {}", e)))?;
        engine_table
            .set("units", units_tbl)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set units: {}", e)))?;

        // Deterministic RNG: seed(n), random()
        let rng_seed = self.rng_state.clone();
        let seed_func = lua
            .create_function(move |_, n: u64| {
                *rng_seed.borrow_mut() = if n == 0 { 0x9E3779B97F4A7C15 } else { n };
                Ok(())
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to create seed: {}", e)))?;
        engine_table
            .set("seed", seed_func)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set seed: {}", e)))?;

        let rng_state = self.rng_state.clone();
        let rand_func = lua
            .create_function(move |_, ()| {
                // xorshift64*
                let mut x = *rng_state.borrow();
                if x == 0 { x = 0x9E3779B97F4A7C15; }
                x ^= x >> 12;
                x ^= x << 25;
                x ^= x >> 27;
                let result = x.wrapping_mul(0x2545F4914F6CDD1D);
                *rng_state.borrow_mut() = x;
                // map to [0,1)
                let val = (result >> 11) as f64 / (1u64 << 53) as f64;
                Ok(val)
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to create random: {}", e)))?;
        engine_table
            .set("random", rand_func)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set random: {}", e)))?;

        // Persistence system
        let store_ref = self.persistence_store.clone();
        let persist_func = match lua.create_function(move |_, (key, value): (String, Value)| {
            tracing::debug!("Persisting key: {}", key);
            store_ref.borrow_mut().insert(key, value);
            Ok(())
        }) {
            Ok(func) => func,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create persist: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("persist", persist_func) {
            return Err(anyhow::Error::msg(format!("Failed to set persist: {}", e)));
        }

        let store_ref2 = self.persistence_store.clone();
        let restore_func = match lua.create_function(move |_, key: String| {
            tracing::debug!("Restoring key: {}", key);
            if let Some(v) = store_ref2.borrow().get(&key) {
                Ok(v.clone())
            } else {
                Ok(Value::Nil)
            }
        }) {
            Ok(func) => func,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create restore: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("restore", restore_func) {
            return Err(anyhow::Error::msg(format!("Failed to set restore: {}", e)));
        }

        // Logging system
        let log_func = match lua.create_function(|_, (level, message): (String, String)| {
            match level.as_str() {
                "info" => tracing::info!("[Lua] {}", message),
                "warn" => tracing::warn!("[Lua] {}", message),
                "error" => tracing::error!("[Lua] {}", message),
                "debug" => tracing::debug!("[Lua] {}", message),
                _ => tracing::info!("[Lua] {}", message),
            }
            Ok(())
        }) {
            Ok(func) => func,
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create log: {}", e))),
        };
        if let Err(e) = engine_table.set("log", log_func) {
            return Err(anyhow::Error::msg(format!("Failed to set log: {}", e)));
        }

        // Metrics access
        let metrics_func = match lua.create_function(|lua, ()| {
            let metrics_table = lua.create_table()?;
            metrics_table.set("cpu_frame_ms", 0.0)?;
            metrics_table.set("ffi_calls", 0)?;
            metrics_table.set("sprites_submitted", 0)?;
            Ok(metrics_table)
        }) {
            Ok(func) => func,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create get_metrics: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("get_metrics", metrics_func) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set get_metrics: {}",
                e
            )));
        }

        // Lock the engine table metatable
        let metatable = match lua.create_table() {
            Ok(table) => table,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create metatable: {}",
                    e
                )))
            }
        };
        if let Err(e) = metatable.set("__metatable", "locked") {
            return Err(anyhow::Error::msg(format!(
                "Failed to set metatable: {}",
                e
            )));
        }
        engine_table.set_metatable(Some(metatable));

        // Set engine namespace globally
        if let Err(e) = globals.set("engine", engine_table) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set engine global: {}",
                e
            )));
        }

        tracing::info!("Engine API namespace initialized (version {})", API_VERSION);
        Ok(())
    }
}

impl Default for EngineApi {
    fn default() -> Self {
        Self::new()
    }
}

impl UserData for EngineApi {}

// Engine-backed transform buffer (stride=6)
pub struct TransformBuffer {
    buf: Rc<RefCell<Vec<f64>>>,
    len: Rc<RefCell<usize>>, // active rows
    cap: Rc<RefCell<usize>>, // capacity in rows
    ppu: Rc<RefCell<f64>>,   // pixels per unit
}

impl TransformBuffer {
    fn new(capacity: usize, ppu: Rc<RefCell<f64>>) -> Self {
        let mut v = Vec::with_capacity(capacity * 6);
        v.resize(capacity * 6, 0.0);
        Self { buf: Rc::new(RefCell::new(v)), len: Rc::new(RefCell::new(0)), cap: Rc::new(RefCell::new(capacity)), ppu }
    }
}

// Typed sprite buffer
#[derive(Clone, Default)]
struct SpriteRow {
    entity_id: u32,
    texture_id: u32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

pub struct SpriteBuffer {
    rows: Rc<RefCell<Vec<SpriteRow>>>,
    len: Rc<RefCell<usize>>, // active rows
    cap: Rc<RefCell<usize>>, // capacity in rows
}

impl SpriteBuffer {
    fn new(capacity: usize) -> Self {
        let mut v = Vec::with_capacity(capacity);
        v.resize_with(capacity, SpriteRow::default);
        Self { rows: Rc::new(RefCell::new(v)), len: Rc::new(RefCell::new(0)), cap: Rc::new(RefCell::new(capacity)) }
    }
}

impl UserData for SpriteBuffer {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("set", |_, this, (i, id_ud, tex_ud, u0, v0, u1, v1, r, g, b, a): (usize, AnyUserData, AnyUserData, f32, f32, f32, f32, f32, f32, f32, f32)| {
            let idx = i.checked_sub(1).ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
            let cap = *this.cap.borrow(); if idx >= cap { return Err(mlua::Error::RuntimeError("index exceeds capacity".into())); }
            let entity_id = id_ud.borrow::<EntityId>()?.0;
            let texture_id = tex_ud.borrow::<TextureHandle>()?.0;
            let mut rows = this.rows.borrow_mut();
            rows[idx] = SpriteRow { entity_id, texture_id, u0, v0, u1, v1, r, g, b, a };
            let mut l = this.len.borrow_mut(); if i > *l { *l = i; }
            Ok(())
        });
        methods.add_method_mut("set_tex", |_, this, (i, id_ud, tex_ud): (usize, AnyUserData, AnyUserData)| {
            let idx = i.checked_sub(1).ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
            let cap = *this.cap.borrow(); if idx >= cap { return Err(mlua::Error::RuntimeError("index exceeds capacity".into())); }
            let entity_id = id_ud.borrow::<EntityId>()?.0;
            let texture_id = tex_ud.borrow::<TextureHandle>()?.0;
            let mut rows = this.rows.borrow_mut();
            let row = &mut rows[idx];
            row.entity_id = entity_id; row.texture_id = texture_id;
            let mut l = this.len.borrow_mut(); if i > *l { *l = i; }
            Ok(())
        });
        methods.add_method_mut("set_uv_rect", |_, this, (i, u0, v0, u1, v1): (usize, f32, f32, f32, f32)| {
            let idx = i.checked_sub(1).ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
            let cap = *this.cap.borrow(); if idx >= cap { return Err(mlua::Error::RuntimeError("index exceeds capacity".into())); }
            let mut rows = this.rows.borrow_mut();
            let row = &mut rows[idx];
            row.u0=u0; row.v0=v0; row.u1=u1; row.v1=v1;
            Ok(())
        });
        methods.add_method_mut("set_color", |_, this, (i, r, g, b, a): (usize, f32, f32, f32, f32)| {
            let idx = i.checked_sub(1).ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
            let cap = *this.cap.borrow(); if idx >= cap { return Err(mlua::Error::RuntimeError("index exceeds capacity".into())); }
            let mut rows = this.rows.borrow_mut();
            let row = &mut rows[idx];
            row.r=r; row.g=g; row.b=b; row.a=a;
            Ok(())
        });
        methods.add_method_mut("set_named_uv", |_, this, (i, atlas_ud, name): (usize, AnyUserData, String)| {
            let idx = i.checked_sub(1).ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
            let cap = *this.cap.borrow(); if idx >= cap { return Err(mlua::Error::RuntimeError("index exceeds capacity".into())); }
            let atlas = atlas_ud.borrow::<Atlas>()?;
            let uv = atlas
                .uv_map
                .get(&name)
                .ok_or_else(|| mlua::Error::RuntimeError(format!("unknown atlas name: {}", name)))?;
            let mut rows = this.rows.borrow_mut();
            let row = &mut rows[idx];
            row.u0 = uv[0]; row.v0 = uv[1]; row.u1 = uv[2]; row.v1 = uv[3];
            row.texture_id = atlas.texture.0;
            Ok(())
        });
        methods.add_method("len", |_, this, ()| Ok(*this.len.borrow() as i64));
        methods.add_method("cap", |_, this, ()| Ok(*this.cap.borrow() as i64));
        methods.add_method_mut("resize", |_, this, new_cap: usize| {
            let mut v = this.rows.borrow_mut(); v.resize_with(new_cap, SpriteRow::default);
            *this.cap.borrow_mut() = new_cap;
            if *this.len.borrow() > new_cap { *this.len.borrow_mut() = new_cap; }
            Ok(())
        });
    }
}

// Atlas object exposed to Lua
#[derive(Clone)]
pub struct Atlas {
    texture: TextureHandle,
    uv_map: HashMap<String, [f32; 4]>,
}

impl UserData for Atlas {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("tex", |_, this, ()| Ok(this.texture));
        methods.add_method("uv", |_, this, name: String| {
            if let Some(uv) = this.uv_map.get(&name) {
                Ok((uv[0], uv[1], uv[2], uv[3]))
            } else {
                Err(mlua::Error::RuntimeError(format!("unknown atlas name: {}", name)))
            }
        });
    }
}
impl UserData for TransformBuffer {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("set", |lua, this, (i, id, x, y, rot, sx, sy): (usize, Value, f64, f64, f64, f64, f64)| {
            let idx = i.checked_sub(1).ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
            let cap = *this.cap.borrow(); if idx >= cap { return Err(mlua::Error::RuntimeError("index exceeds capacity".into())); }
            // id may be EntityId or number
            let entity_id = match id {
                Value::UserData(ud) => {
                    if let Ok(ent) = ud.borrow::<EntityId>() { ent.0 as f64 } else { return Err(mlua::Error::RuntimeError("id must be EntityId or number".into())); }
                }
                v => f64::from_lua(v, lua)?,
            };
            let off = idx * 6; let mut b = this.buf.borrow_mut();
            b[off+0]=entity_id; b[off+1]=x; b[off+2]=y; b[off+3]=rot; b[off+4]=sx; b[off+5]=sy;
            let mut l = this.len.borrow_mut(); if i > *l { *l = i; }
            Ok(())
        });
        methods.add_method_mut("set_px", |lua, this, (i, id, x_px, y_px, rot, w_px, h_px): (usize, Value, f64, f64, f64, f64, f64)| {
            let idx = i.checked_sub(1).ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
            let cap = *this.cap.borrow(); if idx >= cap { return Err(mlua::Error::RuntimeError("index exceeds capacity".into())); }
            let entity_id = match id {
                Value::UserData(ud) => { if let Ok(ent) = ud.borrow::<EntityId>() { ent.0 as f64 } else { return Err(mlua::Error::RuntimeError("id must be EntityId or number".into())); } }
                v => f64::from_lua(v, lua)?,
            };
            let ppu = *this.ppu.borrow();
            let sx = w_px / ppu; let sy = h_px / ppu;
            let off = idx * 6; let mut b = this.buf.borrow_mut();
            b[off+0]=entity_id; b[off+1]=x_px; b[off+2]=y_px; b[off+3]=rot; b[off+4]=sx; b[off+5]=sy;
            let mut l = this.len.borrow_mut(); if i > *l { *l = i; }
            Ok(())
        });
        methods.add_method("len", |_, this, ()| Ok(*this.len.borrow() as i64));
        methods.add_method("cap", |_, this, ()| Ok(*this.cap.borrow() as i64));
        methods.add_method_mut("resize", |_, this, new_cap: usize| {
            let mut v = this.buf.borrow_mut(); v.resize(new_cap*6, 0.0);
            *this.cap.borrow_mut() = new_cap;
            if *this.len.borrow() > new_cap { *this.len.borrow_mut() = new_cap; }
            Ok(())
        });
    }
}

/// Sprite description parsed from v2 array format
#[derive(Debug, Clone)]
pub struct SpriteV2 {
    pub entity_id: u32,
    pub texture_id: u32,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl EngineApi {
    /// Variant that installs callbacks to forward v2 arrays into the host.
    pub fn setup_engine_namespace_with_sinks(
        &self,
        lua: &Lua,
        set_transforms_cb: Rc<dyn Fn(&[f64])>,
        submit_sprites_cb: Rc<dyn Fn(&[SpriteV2])>,
    ) -> Result<()> {
        let globals = lua.globals();

        // Build the base engine table via the normal path first
        self.setup_engine_namespace(lua)?;
        let engine_table: mlua::Table = match globals.get("engine") {
            Ok(t) => t,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to get engine table: {}",
                    e
                )))
            }
        };

        // Override set_transforms (accepts table or TransformBuffer)
        let st_cb = set_transforms_cb.clone();
        let transforms_scratch = std::cell::RefCell::new(Vec::<f64>::with_capacity(1024));
        let transform_func = match lua.create_function(move |lua, arg: Value| {
            match arg {
                Value::UserData(ud) => {
                    if let Ok(tb) = ud.borrow::<TransformBuffer>() {
                        let rows = *tb.len.borrow();
                        let buf = tb.buf.borrow();
                        let take = rows * 6;
                        (st_cb)(&buf[..take]);
                        tracing::debug!("Setting {} transforms (typed)", rows);
                        Ok(())
                    } else {
                        Err(mlua::Error::RuntimeError("ARG_ERROR: unsupported userdata for set_transforms".into()))
                    }
                }
                Value::Table(arr) => {
                    let len = arr.raw_len();
                    if len % 6 != 0 {
                        return Err(mlua::Error::RuntimeError(format!(
                            "ARG_ERROR: set_transforms stride mismatch (got={}, want=6)",
                            len % 6
                        )));
                    }
                    let mut out = transforms_scratch.borrow_mut();
                    out.clear();
                    out.reserve(len);
                    let mut i = 1;
                    while i <= len {
                        // id can be EntityId userdata or a number
                        let v: mlua::Value = arr.raw_get(i)?; i += 1;
                        let id_num = match v {
                            mlua::Value::UserData(ud) => {
                                if let Ok(ent) = ud.borrow::<EntityId>() { ent.0 as f64 } else {
                                    return Err(mlua::Error::RuntimeError("ARG_ERROR: id must be EntityId or number".into()));
                                }
                            }
                            _ => f64::from_lua(v, lua)?,
                        };
                        out.push(id_num);
                        // x,y,rot,sx,sy numbers
                        let x: f64 = arr.raw_get(i)?; i += 1; out.push(x);
                        let y: f64 = arr.raw_get(i)?; i += 1; out.push(y);
                        let rot: f64 = arr.raw_get(i)?; i += 1; out.push(rot);
                        let sx: f64 = arr.raw_get(i)?; i += 1; out.push(sx);
                        let sy: f64 = arr.raw_get(i)?; i += 1; out.push(sy);
                    }
                    tracing::debug!("Setting {} transforms", out.len() / 6);
                    (st_cb)(&out);
                    Ok(())
                }
                _ => Err(mlua::Error::RuntimeError("ARG_ERROR: set_transforms expects table or TransformBuffer".into())),
            }
        }) {
            Ok(f) => f,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create set_transforms: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("set_transforms", transform_func) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set set_transforms: {}",
                e
            )));
        }

        // Override submit_sprites (accepts table or SpriteBuffer) and parse values into SpriteV2
        let sb_cb = submit_sprites_cb.clone();
        let sprites_scratch = std::cell::RefCell::new(Vec::<SpriteV2>::with_capacity(1024));
        let sprite_func = match lua.create_function(move |lua, arg: Value| {
            match arg {
                Value::UserData(ud) => {
                    if let Ok(sb) = ud.borrow::<SpriteBuffer>() {
                        let rows = *sb.len.borrow();
                        let vec = sb.rows.borrow();
                        let mut out = sprites_scratch.borrow_mut();
                        out.clear();
                        out.reserve(rows);
                        for i in 0..rows {
                            let r = &vec[i];
                            out.push(SpriteV2 { entity_id: r.entity_id, texture_id: r.texture_id, u0: r.u0, v0: r.v0, u1: r.u1, v1: r.v1, r: r.r, g: r.g, b: r.b, a: r.a });
                        }
                        tracing::debug!("Submitting {} sprites (typed)", rows);
                        (sb_cb)(&out);
                        Ok(())
                    } else {
                        Err(mlua::Error::RuntimeError("ARG_ERROR: unsupported userdata for submit_sprites".into()))
                    }
                }
                Value::Table(sprites) => {
                    // Accept array table
                    let len = sprites.raw_len();
                    if len % 10 != 0 {
                        return Err(mlua::Error::RuntimeError(format!(
                            "ARG_ERROR: submit_sprites stride mismatch (got={}, want=10)",
                            len % 10
                        )));
                    }
                    let mut out = sprites_scratch.borrow_mut();
                    out.clear();
                    out.reserve(len / 10);
                    let mut i = 1; // Lua arrays are 1-based
                    while i <= len {
                        // entity (UserData EntityId)
                        let ent_ud: mlua::AnyUserData = sprites.raw_get(i)?; i += 1;
                        let entity_id = if let Ok(ent) = ent_ud.borrow::<EntityId>() { ent.0 } else {
                            return Err(mlua::Error::RuntimeError("ARG_ERROR: sprite id must be EntityId".into())); };
                        // texture (UserData TextureHandle)
                        let tex_ud: mlua::AnyUserData = sprites.raw_get(i)?; i += 1;
                        let texture_id = if let Ok(tex) = tex_ud.borrow::<TextureHandle>() { tex.0 } else {
                            return Err(mlua::Error::RuntimeError("ARG_ERROR: texture must be TextureHandle".into())); };
                        // remaining 8 numbers
                        let u0: f32 = sprites.raw_get(i)?; i += 1;
                        let v0: f32 = sprites.raw_get(i)?; i += 1;
                        let u1: f32 = sprites.raw_get(i)?; i += 1;
                        let v1: f32 = sprites.raw_get(i)?; i += 1;
                        let r: f32 = sprites.raw_get(i)?; i += 1;
                        let g: f32 = sprites.raw_get(i)?; i += 1;
                        let b: f32 = sprites.raw_get(i)?; i += 1;
                        let a: f32 = sprites.raw_get(i)?; i += 1;
                        out.push(SpriteV2 { entity_id, texture_id, u0, v0, u1, v1, r, g, b, a });
                    }
                    tracing::debug!("Submitting {} sprites", out.len());
                    let _ = lua; // silence unused warning
                    (sb_cb)(&out);
                    Ok(())
                }
                _ => Err(mlua::Error::RuntimeError("ARG_ERROR: submit_sprites expects table or SpriteBuffer".into())),
            }
        }) {
            Ok(f) => f,
            Err(e) => {
                return Err(anyhow::Error::msg(format!(
                    "Failed to create submit_sprites: {}",
                    e
                )))
            }
        };
        if let Err(e) = engine_table.set("submit_sprites", sprite_func) {
            return Err(anyhow::Error::msg(format!(
                "Failed to set submit_sprites: {}",
                e
            )));
        }

        Ok(())
    }

    /// Extended variant to also override get_metrics via a provider closure.
    pub fn setup_engine_namespace_with_sinks_and_metrics(
        &self,
        lua: &Lua,
        set_transforms_cb: Rc<dyn Fn(&[f64])>,
        submit_sprites_cb: Rc<dyn Fn(&[SpriteV2])>,
        metrics_provider: Rc<dyn Fn() -> (f64, u32, u32)>,
        load_texture_cb: Rc<dyn Fn(String, u32)>,
        input_provider: Rc<dyn Fn() -> InputSnapshot>,
        window_size_provider: Rc<dyn Fn() -> (u32, u32)>,
    ) -> Result<()> {
        // First install base + sinks
        self.setup_engine_namespace_with_sinks(
            lua,
            set_transforms_cb,
            submit_sprites_cb,
        )?;

        // Override get_metrics
        let globals = lua.globals();
        let engine_table: mlua::Table = globals
            .get("engine")
            .map_err(|e| anyhow::Error::msg(format!("Failed to get engine table: {}", e)))?;

        let provider = metrics_provider.clone();
        let metrics_func = lua
            .create_function(move |lua, ()| {
                let (cpu_ms, sprites, ffi) = provider();
                let tbl = lua.create_table()?;
                tbl.set("cpu_frame_ms", cpu_ms)?;
                tbl.set("sprites_submitted", sprites)?;
                tbl.set("ffi_calls", ffi)?;
                Ok(tbl)
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to override get_metrics: {}", e)))?;

        engine_table
            .set("get_metrics", metrics_func)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set get_metrics: {}", e)))?;

        // Override get_input using provider
        let input_p = input_provider.clone();
        let input_func = lua
            .create_function(move |_, ()| Ok(input_p()))
            .map_err(|e| anyhow::Error::msg(format!("Failed to override get_input: {}", e)))?;
        engine_table
            .set("get_input", input_func)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set get_input: {}", e)))?;

        // Add window_size() -> (w, h)
        let ws_p = window_size_provider.clone();
        let ws_func = lua
            .create_function(move |_, ()| {
                let (w, h) = ws_p();
                Ok((w, h))
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to create window_size: {}", e)))?;
        engine_table
            .set("window_size", ws_func)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set window_size: {}", e)))?;

        // Override load_texture to notify host and return a handle immediately
        let next_texture_id = std::cell::RefCell::new(self.next_texture_id);
        let lt_cb = load_texture_cb.clone();
        let load_func = lua
            .create_function(move |_, path: String| {
                let mut id_ref = next_texture_id.borrow_mut();
                let id = *id_ref;
                *id_ref += 1;
                lt_cb(path.clone(), id);
                Ok(TextureHandle(id))
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to override load_texture: {}", e)))?;
        engine_table
            .set("load_texture", load_func)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set load_texture: {}", e)))?;

        // atlas_load(png, json) -> Atlas|nil
        #[derive(Deserialize)]
        struct AtlasJsonEntry { x: f32, y: f32, w: f32, h: f32 }
        #[derive(Deserialize)]
        struct AtlasDoc { frames: HashMap<String, AtlasJsonEntry>, width: Option<f32>, height: Option<f32> }
        let next_tex_for_atlas = std::cell::RefCell::new(self.next_texture_id + 10_000);
        let lt_cb2 = load_texture_cb.clone();
        let atlas_func = lua
            .create_function(move |lua, (png_path, json_path): (String, String)| {
                let mut id_ref = next_tex_for_atlas.borrow_mut();
                let id = *id_ref; *id_ref += 1;
                lt_cb2(png_path.clone(), id);
                let doc_s = match std::fs::read_to_string(&json_path) { Ok(s) => s, Err(_) => return Ok(Value::Nil) };
                let parsed: AtlasDoc = match serde_json::from_str(&doc_s) { Ok(v) => v, Err(_) => return Ok(Value::Nil) };
                let mut uv_map = HashMap::new();
                let sheet_w = parsed.width.unwrap_or(1.0);
                let sheet_h = parsed.height.unwrap_or(1.0);
                for (name, e) in parsed.frames.into_iter() {
                    let u0 = e.x / sheet_w;
                    let v0 = e.y / sheet_h;
                    let u1 = (e.x + e.w) / sheet_w;
                    let v1 = (e.y + e.h) / sheet_h;
                    uv_map.insert(name, [u0, v0, u1, v1]);
                }
                let atlas = Atlas { texture: TextureHandle(id), uv_map };
                Ok(Value::UserData(lua.create_userdata(atlas)?))
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to create atlas_load: {}", e)))?;
        engine_table
            .set("atlas_load", atlas_func)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set atlas_load: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_transforms_stride_error() {
        let lua = Lua::new();
        let api = EngineApi::new();
        api.setup_engine_namespace(&lua).unwrap();
        let globals = lua.globals();
        let engine_tbl: mlua::Table = globals.get("engine").unwrap();
        let func: mlua::Function = engine_tbl.get("set_transforms").unwrap();
        // 5 elements (not divisible by 6)
        let t = lua.create_table().unwrap();
        for (i, v) in [1.0f64, 0.0, 0.0, 0.0, 1.0].iter().enumerate() {
            t.raw_set(i + 1, *v).unwrap();
        }
        let err = func.call::<()>(t).unwrap_err();
        let s = format!("{}", err);
        assert!(
            s.contains("ARG_ERROR: set_transforms stride mismatch"),
            "{}",
            s
        );
    }

    #[test]
    fn submit_sprites_stride_error() {
        let lua = Lua::new();
        let api = EngineApi::new();
        let captured = Rc::new(RefCell::new(Vec::<SpriteV2>::new()));
        let cap2 = captured.clone();
        api.setup_engine_namespace_with_sinks(
            &lua,
            Rc::new(|_| {}),
            Rc::new(move |sprites| {
                *cap2.borrow_mut() = sprites.to_vec();
            }),
        )
        .unwrap();
        let globals = lua.globals();
        let engine_tbl: mlua::Table = globals.get("engine").unwrap();
        let func: mlua::Function = engine_tbl.get("submit_sprites").unwrap();
        // Build a table of 9 elements (invalid stride)
        let t = lua.create_table().unwrap();
        // Need at least first two as userdatas
        let ent = EntityId(1);
        let tex = TextureHandle(1);
        t.raw_set(1, lua.create_userdata(ent).unwrap()).unwrap();
        t.raw_set(2, lua.create_userdata(tex).unwrap()).unwrap();
        // add 7 numbers to make len=9
        for i in 0..7 {
            t.raw_set(3 + i, 0.0f32).unwrap();
        }
        let err = func.call::<()>(t).unwrap_err();
        let s = format!("{}", err);
        assert!(
            s.contains("ARG_ERROR: submit_sprites stride mismatch"),
            "{}",
            s
        );
    }

    #[test]
    fn set_transforms_success_callbacks() {
        let lua = Lua::new();
        let api = EngineApi::new();
        let captured: Rc<RefCell<Option<Vec<f64>>>> = Rc::new(RefCell::new(None));
        let cap2 = captured.clone();
        api.setup_engine_namespace_with_sinks(
            &lua,
            Rc::new(move |v| {
                *cap2.borrow_mut() = Some(v.to_vec());
            }),
            Rc::new(|_| {}),
        )
        .unwrap();
        let globals = lua.globals();
        let engine_tbl: mlua::Table = globals.get("engine").unwrap();
        let func: mlua::Function = engine_tbl.get("set_transforms").unwrap();
        let t = lua.create_table().unwrap();
        // id as number
        for (i, v) in [1.0f64, 10.0, 20.0, 0.0, 1.0, 1.0].iter().enumerate() {
            t.raw_set(i + 1, *v).unwrap();
        }
        func.call::<()>(t).unwrap();
        let got = captured.borrow().clone().unwrap();
        assert_eq!(got.len(), 6);
        assert_eq!(got[1], 10.0);
    }

    #[test]
    fn set_transforms_accepts_entity_id() {
        let lua = Lua::new();
        let api = EngineApi::new();
        let captured: Rc<RefCell<Option<Vec<f64>>>> = Rc::new(RefCell::new(None));
        let cap2 = captured.clone();
        api.setup_engine_namespace_with_sinks(
            &lua,
            Rc::new(move |v| {
                *cap2.borrow_mut() = Some(v.to_vec());
            }),
            Rc::new(|_| {}),
        )
        .unwrap();
        let globals = lua.globals();
        let engine_tbl: mlua::Table = globals.get("engine").unwrap();
        let func: mlua::Function = engine_tbl.get("set_transforms").unwrap();
        // Build table with EntityId as first field
        let t = lua.create_table().unwrap();
        let ent_ud = lua.create_userdata(EntityId(42)).unwrap();
        t.raw_set(1, ent_ud).unwrap();
        t.raw_set(2, 100.0f64).unwrap();
        t.raw_set(3, 200.0f64).unwrap();
        t.raw_set(4, 0.0f64).unwrap();
        t.raw_set(5, 1.0f64).unwrap();
        t.raw_set(6, 1.0f64).unwrap();
        func.call::<()>(t).unwrap();
        let got = captured.borrow().clone().unwrap();
        assert_eq!(got[0], 42.0);
    }

    #[test]
    fn submit_sprites_success_callbacks() {
        let lua = Lua::new();
        let api = EngineApi::new();
        let captured: Rc<RefCell<Vec<SpriteV2>>> = Rc::new(RefCell::new(Vec::new()));
        let cap2 = captured.clone();
        api.setup_engine_namespace_with_sinks(
            &lua,
            Rc::new(|_| {}),
            Rc::new(move |sprites| {
                *cap2.borrow_mut() = sprites.to_vec();
            }),
        )
        .unwrap();
        let globals = lua.globals();
        let engine_tbl: mlua::Table = globals.get("engine").unwrap();
        let func: mlua::Function = engine_tbl.get("submit_sprites").unwrap();

        // Build valid sprite array (stride 10)
        let t = lua.create_table().unwrap();
        let ent = EntityId(7);
        let tex = TextureHandle(3);
        t.raw_set(1, lua.create_userdata(ent).unwrap()).unwrap();
        t.raw_set(2, lua.create_userdata(tex).unwrap()).unwrap();
        // u0,v0,u1,v1,r,g,b,a
        t.raw_set(3, 0.1f32).unwrap();
        t.raw_set(4, 0.2f32).unwrap();
        t.raw_set(5, 0.9f32).unwrap();
        t.raw_set(6, 0.8f32).unwrap();
        t.raw_set(7, 1.0f32).unwrap();
        t.raw_set(8, 0.5f32).unwrap();
        t.raw_set(9, 0.25f32).unwrap();
        t.raw_set(10, 1.0f32).unwrap();

        func.call::<()>(t).unwrap();

        let got = captured.borrow();
        assert_eq!(got.len(), 1);
        let s = &got[0];
        assert_eq!(s.entity_id, 7);
        assert_eq!(s.texture_id, 3);
        assert!((s.u0 - 0.1).abs() < 1e-6);
        assert!((s.v0 - 0.2).abs() < 1e-6);
        assert!((s.u1 - 0.9).abs() < 1e-6);
        assert!((s.v1 - 0.8).abs() < 1e-6);
        assert!((s.r - 1.0).abs() < 1e-6);
        assert!((s.g - 0.5).abs() < 1e-6);
        assert!((s.b - 0.25).abs() < 1e-6);
        assert!((s.a - 1.0).abs() < 1e-6);
    }

}
