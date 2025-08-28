use anyhow::Result;
use mlua::{Lua, UserData, UserDataMethods, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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
    #[allow(dead_code)] // TODO: will be used in persist/restore system
    persistence_store: HashMap<String, Value>,
    #[allow(dead_code)] // TODO: will be used for capability queries
    capabilities: EngineCapabilities,
}

impl EngineApi {
    pub fn new() -> Self {
        Self {
            next_entity_id: 1,
            next_texture_id: 1,
            fixed_time: Rc::new(RefCell::new(0.0)),
            persistence_store: HashMap::new(), // TODO: will be used in persist/restore system
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

        // Transform batching (v2 format)
        let transform_func = match lua.create_function(|_, transforms: Vec<f64>| {
            if transforms.len() % 6 != 0 {
                return Err(mlua::Error::RuntimeError(format!(
                    "ARG_ERROR: set_transforms stride mismatch (got={}, want=6)",
                    transforms.len() % 6
                )));
            }
            tracing::debug!("Setting {} transforms", transforms.len() / 6);
            Ok(())
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

        // Sprite batching (v2 format)
        let sprite_func = match lua.create_function(|_, sprites: Vec<Value>| {
            if sprites.len() % 10 != 0 {
                return Err(mlua::Error::RuntimeError(format!(
                    "ARG_ERROR: submit_sprites stride mismatch (got={}, want=10)",
                    sprites.len() % 10
                )));
            }
            tracing::debug!("Submitting {} sprites", sprites.len() / 10);
            Ok(())
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

        // Persistence system
        let persist_func = match lua.create_function(|_, (key, _value): (String, Value)| {
            tracing::debug!("Persisting key: {}", key);
            // TODO: Store in actual persistence system
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

        let restore_func = match lua.create_function(|_, key: String| {
            tracing::debug!("Restoring key: {}", key);
            // TODO: Restore from actual persistence system
            Ok(Value::Nil)
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
        set_transforms_cb: Rc<dyn Fn(Vec<f64>)>,
        submit_sprites_cb: Rc<dyn Fn(Vec<SpriteV2>)>,
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

        // Override set_transforms
        let st_cb = set_transforms_cb.clone();
        let transform_func = match lua.create_function(move |_, transforms: Vec<f64>| {
            if transforms.len() % 6 != 0 {
                return Err(mlua::Error::RuntimeError(format!(
                    "ARG_ERROR: set_transforms stride mismatch (got={}, want=6)",
                    transforms.len() % 6
                )));
            }
            tracing::debug!("Setting {} transforms", transforms.len() / 6);
            (st_cb)(transforms);
            Ok(())
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

        // Override submit_sprites and parse values into SpriteV2
        let sb_cb = submit_sprites_cb.clone();
        let sprite_func = match lua.create_function(move |lua, sprites: mlua::Table| {
            // Accept either an array table or a Vec<Value>
            let len = sprites.raw_len();
            if len % 10 != 0 {
                return Err(mlua::Error::RuntimeError(format!(
                    "ARG_ERROR: submit_sprites stride mismatch (got={}, want=10)",
                    len % 10
                )));
            }
            let mut out: Vec<SpriteV2> = Vec::with_capacity(len / 10);
            let mut i = 1; // Lua arrays are 1-based
            while i <= len {
                // entity (UserData EntityId)
                let ent_ud: mlua::AnyUserData = sprites.raw_get(i)?;
                i += 1;
                let entity_id = if let Ok(ent) = ent_ud.borrow::<EntityId>() {
                    ent.0
                } else {
                    return Err(mlua::Error::RuntimeError(
                        "ARG_ERROR: sprite id must be EntityId".into(),
                    ));
                };
                // texture (UserData TextureHandle)
                let tex_ud: mlua::AnyUserData = sprites.raw_get(i)?;
                i += 1;
                let texture_id = if let Ok(tex) = tex_ud.borrow::<TextureHandle>() {
                    tex.0
                } else {
                    return Err(mlua::Error::RuntimeError(
                        "ARG_ERROR: texture must be TextureHandle".into(),
                    ));
                };
                // remaining 8 numbers
                let u0: f32 = sprites.raw_get(i)?;
                i += 1;
                let v0: f32 = sprites.raw_get(i)?;
                i += 1;
                let u1: f32 = sprites.raw_get(i)?;
                i += 1;
                let v1: f32 = sprites.raw_get(i)?;
                i += 1;
                let r: f32 = sprites.raw_get(i)?;
                i += 1;
                let g: f32 = sprites.raw_get(i)?;
                i += 1;
                let b: f32 = sprites.raw_get(i)?;
                i += 1;
                let a: f32 = sprites.raw_get(i)?;
                i += 1;
                out.push(SpriteV2 {
                    entity_id,
                    texture_id,
                    u0,
                    v0,
                    u1,
                    v1,
                    r,
                    g,
                    b,
                    a,
                });
            }
            tracing::debug!("Submitting {} sprites", out.len());
            let _ = lua; // silence unused warning
            (sb_cb)(out);
            Ok(())
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
        let err = func
            .call::<()>(vec![1.0f64, 0.0, 0.0, 0.0, 1.0])
            .unwrap_err();
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
                *cap2.borrow_mut() = sprites;
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
                *cap2.borrow_mut() = Some(v);
            }),
            Rc::new(|_| {}),
        )
        .unwrap();
        let globals = lua.globals();
        let engine_tbl: mlua::Table = globals.get("engine").unwrap();
        let func: mlua::Function = engine_tbl.get("set_transforms").unwrap();
        let v = vec![1.0f64, 10.0, 20.0, 0.0, 1.0, 1.0];
        func.call::<()>(v.clone()).unwrap();
        let got = captured.borrow().clone().unwrap();
        assert_eq!(got.len(), 6);
        assert_eq!(got[1], 10.0);
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
                *cap2.borrow_mut() = sprites;
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
