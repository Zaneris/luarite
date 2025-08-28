use mlua::{Lua, UserData, UserDataMethods, Value};
use anyhow::Result;
use std::collections::HashMap;

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

impl UserData for InputSnapshot {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_key", |_, this, key: String| {
            Ok(this.keys.get(&key).copied().unwrap_or(false))
        });
        
        methods.add_method("get_mouse_button", |_, this, button: String| {
            Ok(this.mouse_buttons.get(&button).copied().unwrap_or(false))
        });
        
        methods.add_method("mouse_pos", |_, this, ()| {
            Ok((this.mouse_x, this.mouse_y))
        });
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
        methods.add_method("supports_hot_reload", |_, this, ()| Ok(this.supports_hot_reload));
        methods.add_method("supports_persistence", |_, this, ()| Ok(this.supports_persistence));
        methods.add_method("v2_arrays", |_, this, ()| Ok(this.v2_arrays));
        methods.add_method("v3_packed_blobs", |_, this, ()| Ok(this.v3_packed_blobs));
    }
}

/// Main engine API struct
pub struct EngineApi {
    next_entity_id: u32,
    next_texture_id: u32,
    fixed_time: f64,
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
            fixed_time: 0.0,
            persistence_store: HashMap::new(), // TODO: will be used in persist/restore system
            capabilities: EngineCapabilities::default(), // TODO: will be used for capability queries
        }
    }

    pub fn update_time(&mut self, dt: f64) {
        self.fixed_time += dt;
    }

    pub fn setup_engine_namespace(&self, lua: &Lua) -> Result<()> {
        let globals = lua.globals();
        
        // Create main engine table
        let engine_table = match lua.create_table() {
            Ok(table) => table,
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create engine table: {}", e))),
        };
        
        // API version and capabilities
        if let Err(e) = engine_table.set("api_version", API_VERSION) {
            return Err(anyhow::Error::msg(format!("Failed to set api_version: {}", e)));
        }

        // Set up get_capabilities function
        let caps_func = match lua.create_function(move |_, ()| {
            Ok(EngineCapabilities::default())
        }) {
            Ok(func) => func,
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create get_capabilities: {}", e))),
        };
        if let Err(e) = engine_table.set("get_capabilities", caps_func) {
            return Err(anyhow::Error::msg(format!("Failed to set get_capabilities: {}", e)));
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
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create create_entity: {}", e))),
        };
        if let Err(e) = engine_table.set("create_entity", entity_func) {
            return Err(anyhow::Error::msg(format!("Failed to set create_entity: {}", e)));
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
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create load_texture: {}", e))),
        };
        if let Err(e) = engine_table.set("load_texture", texture_func) {
            return Err(anyhow::Error::msg(format!("Failed to set load_texture: {}", e)));
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
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create set_transforms: {}", e))),
        };
        if let Err(e) = engine_table.set("set_transforms", transform_func) {
            return Err(anyhow::Error::msg(format!("Failed to set set_transforms: {}", e)));
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
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create submit_sprites: {}", e))),
        };
        if let Err(e) = engine_table.set("submit_sprites", sprite_func) {
            return Err(anyhow::Error::msg(format!("Failed to set submit_sprites: {}", e)));
        }

        // Input system
        let input_func = match lua.create_function(|_, ()| {
            Ok(InputSnapshot::new())
        }) {
            Ok(func) => func,
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create get_input: {}", e))),
        };
        if let Err(e) = engine_table.set("get_input", input_func) {
            return Err(anyhow::Error::msg(format!("Failed to set get_input: {}", e)));
        }

        // Time system
        let fixed_time = std::cell::RefCell::new(self.fixed_time);
        let time_func = match lua.create_function(move |_, ()| {
            Ok(*fixed_time.borrow())
        }) {
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
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create persist: {}", e))),
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
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create restore: {}", e))),
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
        let metrics_func = match lua.create_function(|_, ()| {
            let new_lua = mlua::Lua::new();
            let metrics_table = new_lua.create_table()?;
            metrics_table.set("cpu_frame_ms", 0.0)?;
            metrics_table.set("ffi_calls", 0)?;
            metrics_table.set("sprites_submitted", 0)?;
            Ok(metrics_table)
        }) {
            Ok(func) => func,
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create get_metrics: {}", e))),
        };
        if let Err(e) = engine_table.set("get_metrics", metrics_func) {
            return Err(anyhow::Error::msg(format!("Failed to set get_metrics: {}", e)));
        }

        // Lock the engine table metatable
        let metatable = match lua.create_table() {
            Ok(table) => table,
            Err(e) => return Err(anyhow::Error::msg(format!("Failed to create metatable: {}", e))),
        };
        if let Err(e) = metatable.set("__metatable", "locked") {
            return Err(anyhow::Error::msg(format!("Failed to set metatable: {}", e)));
        }
        engine_table.set_metatable(Some(metatable));
        
        // Set engine namespace globally
        if let Err(e) = globals.set("engine", engine_table) {
            return Err(anyhow::Error::msg(format!("Failed to set engine global: {}", e)));
        }

        tracing::info!("Engine API namespace initialized (version {})", API_VERSION);
        Ok(())
    }
}

impl UserData for EngineApi {}