use anyhow::Result;
use engine_core::state::SpriteData;
use mlua::{AnyUserData, FromLua, Lua, RegistryKey, UserData, UserDataMethods, Value};
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use winit::keyboard::KeyCode;

// Type aliases to simplify complex function pointer types for clippy
type SetTransformsCb = Rc<dyn Fn(&[f64])>;
type SetTransformsF32Cb = Option<Rc<dyn Fn(Rc<RefCell<Vec<f32>>>, usize, usize)>>;
type SubmitSpritesCb = Rc<dyn Fn(&[SpriteV2])>;
type SubmitSpritesTypedCb = Option<Rc<dyn Fn(Rc<RefCell<Vec<SpriteData>>>, usize, usize)>>;
type MetricsProviderCb = Rc<dyn Fn() -> (f64, u32, u32)>;
type LoadTextureCb = Rc<dyn Fn(String, u32)>;
type InputProviderCb = Rc<dyn Fn() -> InputSnapshot>;
type WindowSizeProviderCb = Rc<dyn Fn() -> (u32, u32)>;
type HudPrintfCb = Rc<dyn Fn(String)>;
type SetClearColorCb = Rc<dyn Fn(f32, f32, f32, f32)>;
type SetRenderModeCb = Rc<dyn Fn(&'static str)>;

/// Complex tuple type for sprite texture parameters
type SpriteTexParams = (
    usize,
    AnyUserData,
    AnyUserData,
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    Option<f32>,
);

/// Callback configuration for the extended engine namespace
pub struct EngineCallbacks {
    pub set_transforms_cb: SetTransformsCb,
    pub set_transforms_f32_cb: SetTransformsF32Cb,
    pub submit_sprites_cb: SubmitSpritesCb,
    pub submit_sprites_typed_cb: SubmitSpritesTypedCb,
    pub metrics_provider: MetricsProviderCb,
    pub load_texture_cb: LoadTextureCb,
    pub input_provider: InputProviderCb,
    pub window_size_provider: WindowSizeProviderCb,
    pub hud_printf_cb: HudPrintfCb,
    pub set_clear_color_cb: SetClearColorCb,
    pub set_render_mode_cb: SetRenderModeCb,
}

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
    pub keys: HashMap<u32, bool>,
    pub prev_keys: HashMap<u32, bool>,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub mouse_buttons: HashMap<String, bool>,
    pub prev_mouse_buttons: HashMap<String, bool>,
}

impl InputSnapshot {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            prev_keys: HashMap::new(),
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_buttons: HashMap::new(),
            prev_mouse_buttons: HashMap::new(),
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
        // --- Keyboard ---
        methods.add_method("get_key", |_, this, key: u32| {
            Ok(this.keys.get(&key).copied().unwrap_or(false))
        });

        methods.add_method("was_key_pressed", |_, this, key: u32| {
            let is_down = this.keys.get(&key).copied().unwrap_or(false);
            let was_down = this.prev_keys.get(&key).copied().unwrap_or(false);
            Ok(is_down && !was_down)
        });

        methods.add_method("was_key_released", |_, this, key: u32| {
            let is_down = this.keys.get(&key).copied().unwrap_or(false);
            let was_down = this.prev_keys.get(&key).copied().unwrap_or(false);
            Ok(!is_down && was_down)
        });

        // Aliases
        methods.add_method("down", |_, this, key: u32| {
            Ok(this.keys.get(&key).copied().unwrap_or(false))
        });
        methods.add_method("pressed", |_, this, key: u32| {
            let is_down = this.keys.get(&key).copied().unwrap_or(false);
            let was_down = this.prev_keys.get(&key).copied().unwrap_or(false);
            Ok(is_down && !was_down)
        });
        methods.add_method("released", |_, this, key: u32| {
            let is_down = this.keys.get(&key).copied().unwrap_or(false);
            let was_down = this.prev_keys.get(&key).copied().unwrap_or(false);
            Ok(!is_down && was_down)
        });

        // --- Mouse ---
        methods.add_method("get_mouse_button", |_, this, button: String| {
            Ok(this.mouse_buttons.get(&button).copied().unwrap_or(false))
        });

        methods.add_method("was_mouse_button_pressed", |_, this, button: String| {
            let is_down = this.mouse_buttons.get(&button).copied().unwrap_or(false);
            let was_down = this
                .prev_mouse_buttons
                .get(&button)
                .copied()
                .unwrap_or(false);
            Ok(is_down && !was_down)
        });

        methods.add_method("was_mouse_button_released", |_, this, button: String| {
            let is_down = this.mouse_buttons.get(&button).copied().unwrap_or(false);
            let was_down = this
                .prev_mouse_buttons
                .get(&button)
                .copied()
                .unwrap_or(false);
            Ok(!is_down && was_down)
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

fn create_keys_table(lua: &Lua) -> mlua::Result<mlua::Table> {
    let keys = lua.create_table()?;
    keys.set("Backquote", KeyCode::Backquote as u32)?;
    keys.set("Backslash", KeyCode::Backslash as u32)?;
    keys.set("BracketLeft", KeyCode::BracketLeft as u32)?;
    keys.set("BracketRight", KeyCode::BracketRight as u32)?;
    keys.set("Comma", KeyCode::Comma as u32)?;
    keys.set("Digit0", KeyCode::Digit0 as u32)?;
    keys.set("Digit1", KeyCode::Digit1 as u32)?;
    keys.set("Digit2", KeyCode::Digit2 as u32)?;
    keys.set("Digit3", KeyCode::Digit3 as u32)?;
    keys.set("Digit4", KeyCode::Digit4 as u32)?;
    keys.set("Digit5", KeyCode::Digit5 as u32)?;
    keys.set("Digit6", KeyCode::Digit6 as u32)?;
    keys.set("Digit7", KeyCode::Digit7 as u32)?;
    keys.set("Digit8", KeyCode::Digit8 as u32)?;
    keys.set("Digit9", KeyCode::Digit9 as u32)?;
    keys.set("Equal", KeyCode::Equal as u32)?;
    keys.set("IntlBackslash", KeyCode::IntlBackslash as u32)?;
    keys.set("IntlRo", KeyCode::IntlRo as u32)?;
    keys.set("IntlYen", KeyCode::IntlYen as u32)?;
    keys.set("KeyA", KeyCode::KeyA as u32)?;
    keys.set("KeyB", KeyCode::KeyB as u32)?;
    keys.set("KeyC", KeyCode::KeyC as u32)?;
    keys.set("KeyD", KeyCode::KeyD as u32)?;
    keys.set("KeyE", KeyCode::KeyE as u32)?;
    keys.set("KeyF", KeyCode::KeyF as u32)?;
    keys.set("KeyG", KeyCode::KeyG as u32)?;
    keys.set("KeyH", KeyCode::KeyH as u32)?;
    keys.set("KeyI", KeyCode::KeyI as u32)?;
    keys.set("KeyJ", KeyCode::KeyJ as u32)?;
    keys.set("KeyK", KeyCode::KeyK as u32)?;
    keys.set("KeyL", KeyCode::KeyL as u32)?;
    keys.set("KeyM", KeyCode::KeyM as u32)?;
    keys.set("KeyN", KeyCode::KeyN as u32)?;
    keys.set("KeyO", KeyCode::KeyO as u32)?;
    keys.set("KeyP", KeyCode::KeyP as u32)?;
    keys.set("KeyQ", KeyCode::KeyQ as u32)?;
    keys.set("KeyR", KeyCode::KeyR as u32)?;
    keys.set("KeyS", KeyCode::KeyS as u32)?;
    keys.set("KeyT", KeyCode::KeyT as u32)?;
    keys.set("KeyU", KeyCode::KeyU as u32)?;
    keys.set("KeyV", KeyCode::KeyV as u32)?;
    keys.set("KeyW", KeyCode::KeyW as u32)?;
    keys.set("KeyX", KeyCode::KeyX as u32)?;
    keys.set("KeyY", KeyCode::KeyY as u32)?;
    keys.set("KeyZ", KeyCode::KeyZ as u32)?;
    keys.set("Minus", KeyCode::Minus as u32)?;
    keys.set("Period", KeyCode::Period as u32)?;
    keys.set("Quote", KeyCode::Quote as u32)?;
    keys.set("Semicolon", KeyCode::Semicolon as u32)?;
    keys.set("Slash", KeyCode::Slash as u32)?;
    keys.set("AltLeft", KeyCode::AltLeft as u32)?;
    keys.set("AltRight", KeyCode::AltRight as u32)?;
    keys.set("Backspace", KeyCode::Backspace as u32)?;
    keys.set("CapsLock", KeyCode::CapsLock as u32)?;
    keys.set("ContextMenu", KeyCode::ContextMenu as u32)?;
    keys.set("ControlLeft", KeyCode::ControlLeft as u32)?;
    keys.set("ControlRight", KeyCode::ControlRight as u32)?;
    keys.set("Enter", KeyCode::Enter as u32)?;
    keys.set("SuperLeft", KeyCode::SuperLeft as u32)?;
    keys.set("SuperRight", KeyCode::SuperRight as u32)?;
    keys.set("ShiftLeft", KeyCode::ShiftLeft as u32)?;
    keys.set("ShiftRight", KeyCode::ShiftRight as u32)?;
    keys.set("Space", KeyCode::Space as u32)?;
    keys.set("Tab", KeyCode::Tab as u32)?;
    keys.set("ArrowDown", KeyCode::ArrowDown as u32)?;
    keys.set("ArrowLeft", KeyCode::ArrowLeft as u32)?;
    keys.set("ArrowRight", KeyCode::ArrowRight as u32)?;
    keys.set("ArrowUp", KeyCode::ArrowUp as u32)?;
    keys.set("End", KeyCode::End as u32)?;
    keys.set("Home", KeyCode::Home as u32)?;
    keys.set("PageDown", KeyCode::PageDown as u32)?;
    keys.set("PageUp", KeyCode::PageUp as u32)?;
    keys.set("F1", KeyCode::F1 as u32)?;
    keys.set("F2", KeyCode::F2 as u32)?;
    keys.set("F3", KeyCode::F3 as u32)?;
    keys.set("F4", KeyCode::F4 as u32)?;
    keys.set("F5", KeyCode::F5 as u32)?;
    keys.set("F6", KeyCode::F6 as u32)?;
    keys.set("F7", KeyCode::F7 as u32)?;
    keys.set("F8", KeyCode::F8 as u32)?;
    keys.set("F9", KeyCode::F9 as u32)?;
    keys.set("F10", KeyCode::F10 as u32)?;
    keys.set("F11", KeyCode::F11 as u32)?;
    keys.set("F12", KeyCode::F12 as u32)?;
    keys.set("Numpad0", KeyCode::Numpad0 as u32)?;
    keys.set("Numpad1", KeyCode::Numpad1 as u32)?;
    keys.set("Numpad2", KeyCode::Numpad2 as u32)?;
    keys.set("Numpad3", KeyCode::Numpad3 as u32)?;
    keys.set("Numpad4", KeyCode::Numpad4 as u32)?;
    keys.set("Numpad5", KeyCode::Numpad5 as u32)?;
    keys.set("Numpad6", KeyCode::Numpad6 as u32)?;
    keys.set("Numpad7", KeyCode::Numpad7 as u32)?;
    keys.set("Numpad8", KeyCode::Numpad8 as u32)?;
    keys.set("Numpad9", KeyCode::Numpad9 as u32)?;
    keys.set("NumpadAdd", KeyCode::NumpadAdd as u32)?;
    keys.set("NumpadDecimal", KeyCode::NumpadDecimal as u32)?;
    keys.set("NumpadDivide", KeyCode::NumpadDivide as u32)?;
    keys.set("NumpadMultiply", KeyCode::NumpadMultiply as u32)?;
    keys.set("NumpadSubtract", KeyCode::NumpadSubtract as u32)?;
    keys.set("NumpadEnter", KeyCode::NumpadEnter as u32)?;
    Ok(keys)
}

/// Main engine API struct
pub struct EngineApi {
    next_entity_id: u32,
    next_texture_id: u32,
    fixed_time: Rc<RefCell<f64>>, // shared with time() closure
    persistence_store: Rc<RefCell<HashMap<String, Value>>>,
    rng_state: Rc<RefCell<u64>>,       // deterministic RNG
    input: Rc<RefCell<InputSnapshot>>, // Shared input state

    // Simple rate limiters (window start, count)
    log_rl: Rc<RefCell<(f64, u32)>>,
    hud_rl: Rc<RefCell<(f64, u32)>>,
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
            input: Rc::new(RefCell::new(InputSnapshot::new())),
            log_rl: Rc::new(RefCell::new((0.0, 0))),
            hud_rl: Rc::new(RefCell::new((0.0, 0))),
            capabilities: EngineCapabilities::default(), // TODO: will be used for capability queries
        }
    }

    pub fn update_time(&mut self, dt: f64) {
        *self.fixed_time.borrow_mut() += dt;
    }

    pub fn setup_engine_namespace(&self, lua: &Lua) -> Result<()> {
        let globals = lua.globals();

        // Create main engine table
        let engine_table = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Key constants
        let keys_table = create_keys_table(lua).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("keys", keys_table)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Constructors: typed buffers
        {
            let tb_ctor = lua
                .create_function(move |_, cap: usize| Ok(TransformBuffer::new(cap)))
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            engine_table
                .set("create_transform_buffer", tb_ctor)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;

            let sb_ctor = lua
                .create_function(move |_, cap: usize| Ok(SpriteBuffer::new(cap)))
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            engine_table
                .set("create_sprite_buffer", sb_ctor)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }

        // API version and capabilities
        engine_table
            .set("api_version", API_VERSION)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Set up get_capabilities function
        let caps_func = lua
            .create_function(move |_, ()| Ok(EngineCapabilities::default()))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("get_capabilities", caps_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Entity management
        let next_entity_id = std::cell::RefCell::new(self.next_entity_id);
        let entity_func = lua
            .create_function(move |_, ()| {
                let mut id = next_entity_id.borrow_mut();
                let entity = EntityId(*id);
                *id += 1;
                Ok(entity)
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("create_entity", entity_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Texture loading
        let next_texture_id = std::cell::RefCell::new(self.next_texture_id);
        let texture_func = lua
            .create_function(move |_, path: String| {
                tracing::info!("Loading texture: {}", path);
                let mut id = next_texture_id.borrow_mut();
                let texture = TextureHandle(*id);
                *id += 1;
                Ok(texture)
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("load_texture", texture_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Transform batching (typed buffers only)
        let transform_func = lua
            .create_function(|_, v: Value| match v {
                Value::UserData(ud) => {
                    if let Ok(tb) = ud.borrow::<TransformBuffer>() {
                        let rows = *tb.len.borrow();
                        let buf = tb.buf.borrow();
                        if rows * 6 > buf.len() {
                            return Err(mlua::Error::RuntimeError(
                                "TransformBuffer length exceeds capacity".into(),
                            ));
                        }
                        tracing::debug!("Setting {} transforms (typed)", rows);
                        Ok(())
                    } else {
                        Err(mlua::Error::RuntimeError(
                            "ARG_ERROR: set_transforms expects TransformBuffer".into(),
                        ))
                    }
                }
                _ => Err(mlua::Error::RuntimeError(
                    "ARG_ERROR: set_transforms expects TransformBuffer".into(),
                )),
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("set_transforms", transform_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Sprite batching (typed buffers only)
        let sprite_func = lua
            .create_function(|_, v: Value| match v {
                Value::UserData(ud) => {
                    if let Ok(_sb) = ud.borrow::<SpriteBuffer>() {
                        tracing::debug!("Submitting sprites (typed)");
                        Ok(())
                    } else {
                        Err(mlua::Error::RuntimeError(
                            "ARG_ERROR: submit_sprites expects SpriteBuffer".into(),
                        ))
                    }
                }
                _ => Err(mlua::Error::RuntimeError(
                    "ARG_ERROR: submit_sprites expects SpriteBuffer".into(),
                )),
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("submit_sprites", sprite_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Frame builder façade: engine.frame_builder(T, S) -> builder
        let fb_ctor = lua
            .create_function(move |lua, (t, s): (AnyUserData, AnyUserData)| {
                let fb = FrameBuilder::new(lua, t, s)?;
                let ud = lua.create_userdata(fb)?;
                Ok(ud)
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("frame_builder", fb_ctor)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Input system
        let input_snapshot = self.input.clone();
        let input_func = lua
            .create_function(move |_, ()| Ok(input_snapshot.borrow().clone()))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("get_input", input_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Time system
        let fixed_time = self.fixed_time.clone();
        let time_func = lua
            .create_function(move |_, ()| Ok(*fixed_time.borrow()))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("time", time_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Deterministic RNG: seed(n), random()
        let rng_seed = self.rng_state.clone();
        let seed_func = lua
            .create_function(move |_, n: u64| {
                *rng_seed.borrow_mut() = if n == 0 { 0x9E3779B97F4A7C15 } else { n };
                Ok(())
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("seed", seed_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let rng_state = self.rng_state.clone();
        let rand_func = lua
            .create_function(move |_, ()| {
                // xorshift64*
                let mut x = *rng_state.borrow();
                if x == 0 {
                    x = 0x9E3779B97F4A7C15;
                }
                x ^= x >> 12;
                x ^= x << 25;
                x ^= x >> 27;
                let result = x.wrapping_mul(0x2545F4914F6CDD1D);
                *rng_state.borrow_mut() = x;
                // map to [0,1)
                let val = (result >> 11) as f64 / (1u64 << 53) as f64;
                Ok(val)
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("random", rand_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Add random_bool(p)
        let rng_state_bool = self.rng_state.clone();
        let rand_bool_func = lua
            .create_function(move |_, p: Option<f64>| {
                // xorshift64*
                let mut x = *rng_state_bool.borrow();
                if x == 0 {
                    x = 0x9E3779B97F4A7C15;
                }
                x ^= x >> 12;
                x ^= x << 25;
                x ^= x >> 27;
                let result = x.wrapping_mul(0x2545F4914F6CDD1D);
                *rng_state_bool.borrow_mut() = x;
                // map to [0,1)
                let val = (result >> 11) as f64 / (1u64 << 53) as f64;
                Ok(val < p.unwrap_or(0.5))
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("random_bool", rand_bool_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Add random_range(min, max)
        let rng_state_range = self.rng_state.clone();
        let rand_range_func = lua
            .create_function(move |_, (min, max): (f64, f64)| {
                // xorshift64*
                let mut x = *rng_state_range.borrow();
                if x == 0 {
                    x = 0x9E3779B97F4A7C15;
                }
                x ^= x >> 12;
                x ^= x << 25;
                x ^= x >> 27;
                let result = x.wrapping_mul(0x2545F4914F6CDD1D);
                *rng_state_range.borrow_mut() = x;
                // map to [0,1)
                let val = (result >> 11) as f64 / (1u64 << 53) as f64;
                Ok(min + val * (max - min))
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("random_range", rand_range_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Persistence system
        let store_ref = self.persistence_store.clone();
        let persist_func = lua
            .create_function(move |_, (key, value): (String, Value)| {
                tracing::debug!("Persisting key: {}", key);
                store_ref.borrow_mut().insert(key, value);
                Ok(())
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("persist", persist_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let store_ref2 = self.persistence_store.clone();
        let restore_func = lua
            .create_function(move |_, key: String| {
                tracing::debug!("Restoring key: {}", key);
                if let Some(v) = store_ref2.borrow().get(&key) {
                    Ok(v.clone())
                } else {
                    Ok(Value::Nil)
                }
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("restore", restore_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Logging system (rate-limited 30 msgs/sec)
        let fixed_time_for_log = self.fixed_time.clone();
        let log_rl = self.log_rl.clone();
        let log_func = lua
            .create_function(move |_, (level, message): (String, String)| {
                let now = *fixed_time_for_log.borrow();
                let mut rl = log_rl.borrow_mut();
                if now - rl.0 >= 1.0 {
                    rl.0 = now;
                    rl.1 = 0;
                }
                if rl.1 < 30 {
                    rl.1 += 1;
                    match level.as_str() {
                        "info" => tracing::info!("[Lua] {}", message),
                        "warn" => tracing::warn!("[Lua] {}", message),
                        "error" => tracing::error!("[Lua] {}", message),
                        "debug" => tracing::debug!("[Lua] {}", message),
                        _ => tracing::info!("[Lua] {}", message),
                    }
                }
                Ok(())
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("log", log_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // --- Dummy implementations for base API ---

        // Metrics access
        let metrics_func = lua
            .create_function(|lua, ()| {
                let metrics_table = lua.create_table()?;
                metrics_table.set("cpu_frame_ms", 0.0)?;
                metrics_table.set("ffi_calls", 0)?;
                metrics_table.set("sprites_submitted", 0)?;
                Ok(metrics_table)
            })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("get_metrics", metrics_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Window size
        let ws_func = lua
            .create_function(|_, ()| Ok((0, 0)))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("window_size", ws_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // HUD printf
        let hud_fn = lua
            .create_function(|_, _: String| Ok(()))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("hud_printf", hud_fn)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Clear color
        let set_clear_color_fn = lua
            .create_function(|_, _: mlua::Variadic<mlua::Value>| Ok(()))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("set_clear_color", set_clear_color_fn)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Render resolution
        let set_render_fn = lua
            .create_function(|_, _: String| Ok(()))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("set_render_resolution", set_render_fn)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Atlas load
        let atlas_func = lua
            .create_function(|_, _: (String, String)| Ok(Value::Nil))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table
            .set("atlas_load", atlas_func)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Lock the engine table metatable
        let metatable = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        metatable
            .set("__metatable", "locked")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        engine_table.set_metatable(Some(metatable));

        // Set engine namespace globally
        globals
            .set("engine", engine_table)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

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

// Engine-backed transform buffer (stride=6, f32)
pub struct TransformBuffer {
    buf: Rc<RefCell<Vec<f32>>>,
    len: Rc<RefCell<usize>>, // active rows
    cap: Rc<RefCell<usize>>, // capacity in rows
}

impl TransformBuffer {
    fn new(capacity: usize) -> Self {
        let mut v = Vec::with_capacity(capacity * 6);
        v.resize(capacity * 6, 0.0);
        Self {
            buf: Rc::new(RefCell::new(v)),
            len: Rc::new(RefCell::new(0)),
            cap: Rc::new(RefCell::new(capacity)),
        }
    }
}

pub struct SpriteBuffer {
    rows: Rc<RefCell<Vec<SpriteData>>>,
    len: Rc<RefCell<usize>>, // active rows
    cap: Rc<RefCell<usize>>, // capacity in rows
}

impl SpriteBuffer {
    fn new(capacity: usize) -> Self {
        let mut v: Vec<SpriteData> = Vec::with_capacity(capacity);
        v.resize(
            capacity,
            SpriteData {
                entity_id: 0,
                texture_id: 0,
                uv: [0.0; 4],
                color: [0.0; 4],
                z: 0.0,
            },
        );
        Self {
            rows: Rc::new(RefCell::new(v)),
            len: Rc::new(RefCell::new(0)),
            cap: Rc::new(RefCell::new(capacity)),
        }
    }
}

impl UserData for SpriteBuffer {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut(
            "set",
            |_,
             this,
             (i, id_ud, tex_ud, u0, v0, u1, v1, r, g, b, a, z): (
                usize,
                AnyUserData,
                AnyUserData,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
            )| {
                let idx = i
                    .checked_sub(1)
                    .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
                let cap = *this.cap.borrow();
                if idx >= cap {
                    return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
                }
                let entity_id = id_ud.borrow::<EntityId>()?.0;
                let texture_id = tex_ud.borrow::<TextureHandle>()?.0;
                let mut rows = this.rows.borrow_mut();
                rows[idx] = SpriteData {
                    entity_id,
                    texture_id,
                    uv: [u0, v0, u1, v1],
                    color: [r, g, b, a],
                    z,
                };
                let mut l = this.len.borrow_mut();
                if i > *l {
                    *l = i;
                }
                Ok(())
            },
        );
        methods.add_method_mut(
            "set_tex",
            |_, this, (i, id_ud, tex_ud): (usize, AnyUserData, AnyUserData)| {
                let idx = i
                    .checked_sub(1)
                    .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
                let cap = *this.cap.borrow();
                if idx >= cap {
                    return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
                }
                let entity_id = id_ud.borrow::<EntityId>()?.0;
                let texture_id = tex_ud.borrow::<TextureHandle>()?.0;
                let mut rows = this.rows.borrow_mut();
                let row = &mut rows[idx];
                row.entity_id = entity_id;
                row.texture_id = texture_id;
                let mut l = this.len.borrow_mut();
                if i > *l {
                    *l = i;
                }
                Ok(())
            },
        );
        methods.add_method_mut(
            "set_uv_rect",
            |_, this, (i, u0, v0, u1, v1): (usize, f32, f32, f32, f32)| {
                let idx = i
                    .checked_sub(1)
                    .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
                let cap = *this.cap.borrow();
                if idx >= cap {
                    return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
                }
                let mut rows = this.rows.borrow_mut();
                let row = &mut rows[idx];
                row.uv = [u0, v0, u1, v1];
                Ok(())
            },
        );
        methods.add_method_mut(
            "set_color",
            |_, this, (i, r, g, b, a): (usize, f32, f32, f32, f32)| {
                let idx = i
                    .checked_sub(1)
                    .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
                let cap = *this.cap.borrow();
                if idx >= cap {
                    return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
                }
                let mut rows = this.rows.borrow_mut();
                let row = &mut rows[idx];
                row.color = [r, g, b, a];
                Ok(())
            },
        );
        methods.add_method_mut("set_z", |_, this, (i, z): (usize, f32)| {
            let idx = i
                .checked_sub(1)
                .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
            let cap = *this.cap.borrow();
            if idx >= cap {
                return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
            }
            let mut rows = this.rows.borrow_mut();
            let row = &mut rows[idx];
            row.z = z;
            Ok(())
        });
        methods.add_method_mut(
            "set_named_uv",
            |_, this, (i, atlas_ud, name): (usize, AnyUserData, String)| {
                let idx = i
                    .checked_sub(1)
                    .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
                let cap = *this.cap.borrow();
                if idx >= cap {
                    return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
                }
                let atlas = atlas_ud.borrow::<Atlas>()?;
                let uv = atlas.uv_map.get(&name).ok_or_else(|| {
                    mlua::Error::RuntimeError(format!("unknown atlas name: {}", name))
                })?;
                let mut rows = this.rows.borrow_mut();
                let row = &mut rows[idx];
                row.uv = *uv;
                row.texture_id = atlas.texture.0;
                Ok(())
            },
        );
        methods.add_method("len", |_, this, ()| Ok(*this.len.borrow() as i64));
        methods.add_method("cap", |_, this, ()| Ok(*this.cap.borrow() as i64));
        methods.add_method_mut("resize", |_, this, new_cap: usize| {
            let mut v = this.rows.borrow_mut();
            v.resize(
                new_cap,
                SpriteData {
                    entity_id: 0,
                    texture_id: 0,
                    uv: [0.0; 4],
                    color: [0.0; 4],
                    z: 0.0,
                },
            );
            *this.cap.borrow_mut() = new_cap;
            if *this.len.borrow() > new_cap {
                *this.len.borrow_mut() = new_cap;
            }
            Ok(())
        });
        methods.add_method_mut("clear", |_, this, ()| {
            *this.len.borrow_mut() = 0;
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
                Err(mlua::Error::RuntimeError(format!(
                    "unknown atlas name: {}",
                    name
                )))
            }
        });
    }
}
impl UserData for TransformBuffer {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut(
            "set",
            |lua, this, (i, id, x, y, rot, w, h): (usize, Value, f64, f64, f64, f64, f64)| {
                let idx = i
                    .checked_sub(1)
                    .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
                let cap = *this.cap.borrow();
                if idx >= cap {
                    return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
                }
                let entity_id = match id {
                    Value::UserData(ud) => {
                        if let Ok(ent) = ud.borrow::<EntityId>() {
                            ent.0 as f32
                        } else {
                            return Err(mlua::Error::RuntimeError(
                                "id must be EntityId or number".into(),
                            ));
                        }
                    }
                    v => f64::from_lua(v, lua)? as f32,
                };
                let off = idx * 6;
                let mut b = this.buf.borrow_mut();
                b[off] = entity_id;
                b[off + 1] = x as f32;
                b[off + 2] = y as f32;
                b[off + 3] = rot as f32;
                b[off + 4] = w as f32;
                b[off + 5] = h as f32;
                let mut l = this.len.borrow_mut();
                if i > *l {
                    *l = i;
                }
                Ok(())
            },
        );
        methods.add_method("len", |_, this, ()| Ok(*this.len.borrow() as i64));
        methods.add_method("cap", |_, this, ()| Ok(*this.cap.borrow() as i64));
        methods.add_method_mut("resize", |_, this, new_cap: usize| {
            let mut v = this.buf.borrow_mut();
            v.resize(new_cap * 6, 0.0);
            *this.cap.borrow_mut() = new_cap;
            if *this.len.borrow() > new_cap {
                *this.len.borrow_mut() = new_cap;
            }
            Ok(())
        });
        methods.add_method_mut("clear", |_, this, ()| {
            *this.len.borrow_mut() = 0;
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
    pub z: f32,
}

impl EngineApi {
    /// Variant that installs callbacks to forward v2 arrays into the host.
    pub fn setup_engine_namespace_with_sinks(
        &self,
        lua: &Lua,
        set_transforms_cb: SetTransformsCb,
        set_transforms_f32_cb: SetTransformsF32Cb,
        submit_sprites_cb: SubmitSpritesCb,
        submit_sprites_typed_cb: SubmitSpritesTypedCb,
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
                        let cap = *tb.cap.borrow();
                        if let Some(cb) = &set_transforms_f32_cb {
                            (cb)(tb.buf.clone(), rows, cap);
                        } else {
                            // Fallback: convert to f64 for legacy callback
                            let buf = tb.buf.borrow();
                            let take = rows * 6;
                            let tmp: Vec<f64> = buf[..take].iter().map(|v| *v as f64).collect();
                            (st_cb)(&tmp);
                        }
                        tracing::debug!("Setting {} transforms (typed)", rows);
                        Ok(())
                    } else {
                        Err(mlua::Error::RuntimeError(
                            "ARG_ERROR: unsupported userdata for set_transforms".into(),
                        ))
                    }
                }
                Value::Table(arr) => {
                    let mut out = transforms_scratch.borrow_mut();
                    out.clear();
                    parse_transforms_table_to_out(lua, arr, &mut out)?;
                    tracing::debug!("Setting {} transforms", out.len() / 6);
                    (st_cb)(&out);
                    Ok(())
                }
                _ => Err(mlua::Error::RuntimeError(
                    "ARG_ERROR: set_transforms expects table or TransformBuffer".into(),
                )),
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
        let sb_typed = submit_sprites_typed_cb.clone();
        let sprites_scratch = std::cell::RefCell::new(Vec::<SpriteV2>::with_capacity(1024));
        let sprite_func = match lua.create_function(move |lua, arg: Value| {
            match arg {
                Value::UserData(ud) => {
                    if let Ok(sb) = ud.borrow::<SpriteBuffer>() {
                        let rows = *sb.len.borrow();
                        let cap = *sb.cap.borrow();
                        if let Some(cb) = &sb_typed {
                            (cb)(sb.rows.clone(), rows, cap);
                        } else {
                            let vec = sb.rows.borrow();
                            let mut out = sprites_scratch.borrow_mut();
                            out.clear();
                            out.reserve(rows);
                            for i in 0..rows {
                                let r = &vec[i];
                                out.push(SpriteV2 {
                                    entity_id: r.entity_id,
                                    texture_id: r.texture_id,
                                    u0: r.uv[0],
                                    v0: r.uv[1],
                                    u1: r.uv[2],
                                    v1: r.uv[3],
                                    r: r.color[0],
                                    g: r.color[1],
                                    b: r.color[2],
                                    a: r.color[3],
                                    z: r.z,
                                });
                            }
                            (sb_cb)(&out);
                        }
                        Ok(())
                    } else {
                        Err(mlua::Error::RuntimeError(
                            "ARG_ERROR: unsupported userdata for submit_sprites".into(),
                        ))
                    }
                }
                Value::Table(sprites) => {
                    let mut out = sprites_scratch.borrow_mut();
                    out.clear();
                    parse_sprites_table_to_out(lua, sprites, &mut out)?;
                    tracing::debug!("Submitting {} sprites", out.len());
                    let _ = lua; // silence unused warning
                    (sb_cb)(&out);
                    Ok(())
                }
                _ => Err(mlua::Error::RuntimeError(
                    "ARG_ERROR: submit_sprites expects table or SpriteBuffer".into(),
                )),
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
        callbacks: EngineCallbacks,
    ) -> Result<()> {
        // First install base + sinks
        self.setup_engine_namespace_with_sinks(
            lua,
            callbacks.set_transforms_cb,
            callbacks.set_transforms_f32_cb,
            callbacks.submit_sprites_cb,
            callbacks.submit_sprites_typed_cb,
        )?;

        // Override get_metrics
        let globals = lua.globals();
        let engine_table: mlua::Table = globals
            .get("engine")
            .map_err(|e| anyhow::Error::msg(format!("Failed to get engine table: {}", e)))?;

        let provider = callbacks.metrics_provider.clone();
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
        let input_p = callbacks.input_provider.clone();
        let input_func = lua
            .create_function(move |_, ()| Ok(input_p()))
            .map_err(|e| anyhow::Error::msg(format!("Failed to override get_input: {}", e)))?;
        engine_table
            .set("get_input", input_func)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set get_input: {}", e)))?;

        // Add window_size() -> (w, h)
        let ws_p = callbacks.window_size_provider.clone();
        let ws_func = lua
            .create_function(move |_, ()| {
                let (w, h) = ws_p();
                Ok((w, h))
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to create window_size: {}", e)))?;
        engine_table
            .set("window_size", ws_func)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set window_size: {}", e)))?;

        // HUD printf hook: engine.hud_printf(msg) (rate-limited 30 msgs/sec)
        let hud_cb = callbacks.hud_printf_cb.clone();
        let hud_rl = self.hud_rl.clone();
        let fixed_time_for_hud = self.fixed_time.clone();
        let hud_fn = lua
            .create_function(move |_, msg: String| {
                let now = *fixed_time_for_hud.borrow();
                let mut rl = hud_rl.borrow_mut();
                if now - rl.0 >= 1.0 {
                    rl.0 = now;
                    rl.1 = 0;
                }
                if rl.1 < 30 {
                    rl.1 += 1;
                    hud_cb(msg);
                }
                Ok(())
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to create hud_printf: {}", e)))?;
        engine_table
            .set("hud_printf", hud_fn)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set hud_printf: {}", e)))?;

        // Add set_clear_color(r, g, b, a?) -> sets background; alpha optional (default 1)
        let scc = callbacks.set_clear_color_cb.clone();
        let set_clear_color_fn = lua
            .create_function(move |lua, args: mlua::Variadic<mlua::Value>| {
                if args.len() < 3 {
                    return Err(mlua::Error::RuntimeError(
                        "ARG_ERROR: set_clear_color expects 3 or 4 numbers".into(),
                    ));
                }
                let r: f32 = f32::from_lua(args[0].clone(), lua)?;
                let g: f32 = f32::from_lua(args[1].clone(), lua)?;
                let b: f32 = f32::from_lua(args[2].clone(), lua)?;
                let a: f32 = if args.len() >= 4 {
                    f32::from_lua(args[3].clone(), lua)?
                } else {
                    1.0
                };
                scc(r, g, b, a);
                Ok(())
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to create set_clear_color: {}", e)))?;
        engine_table
            .set("set_clear_color", set_clear_color_fn)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set set_clear_color: {}", e)))?;

        // Add set_render_resolution("retro"|"hd")
        let srm = callbacks.set_render_mode_cb.clone();
        let set_render_fn = lua
            .create_function(move |_, mode: String| {
                let m = if mode.eq_ignore_ascii_case("retro") {
                    "retro"
                } else {
                    "hd"
                };
                // static str coercion for callback type simplicity
                if m == "retro" {
                    srm("retro")
                } else {
                    srm("hd")
                }
                Ok(())
            })
            .map_err(|e| {
                anyhow::Error::msg(format!("Failed to create set_render_resolution: {}", e))
            })?;
        engine_table
            .set("set_render_resolution", set_render_fn)
            .map_err(|e| {
                anyhow::Error::msg(format!("Failed to set set_render_resolution: {}", e))
            })?;

        // Override load_texture to notify host and return a handle immediately
        let next_texture_id = std::cell::RefCell::new(self.next_texture_id);
        let lt_cb = callbacks.load_texture_cb.clone();
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
        struct AtlasJsonEntry {
            x: f32,
            y: f32,
            w: f32,
            h: f32,
        }
        #[derive(Deserialize)]
        struct AtlasDoc {
            frames: HashMap<String, AtlasJsonEntry>,
            width: Option<f32>,
            height: Option<f32>,
        }
        let next_tex_for_atlas = std::cell::RefCell::new(self.next_texture_id + 10_000);
        let lt_cb2 = callbacks.load_texture_cb.clone();
        let atlas_func = lua
            .create_function(move |lua, (png_path, json_path): (String, String)| {
                let mut id_ref = next_tex_for_atlas.borrow_mut();
                let id = *id_ref;
                *id_ref += 1;
                lt_cb2(png_path.clone(), id);
                let doc_s = match std::fs::read_to_string(&json_path) {
                    Ok(s) => s,
                    Err(_) => return Ok(Value::Nil),
                };
                let parsed: AtlasDoc = match serde_json::from_str(&doc_s) {
                    Ok(v) => v,
                    Err(_) => return Ok(Value::Nil),
                };
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
                let atlas = Atlas {
                    texture: TextureHandle(id),
                    uv_map,
                };
                Ok(Value::UserData(lua.create_userdata(atlas)?))
            })
            .map_err(|e| anyhow::Error::msg(format!("Failed to create atlas_load: {}", e)))?;
        engine_table
            .set("atlas_load", atlas_func)
            .map_err(|e| anyhow::Error::msg(format!("Failed to set atlas_load: {}", e)))?;

        Ok(())
    }
}

// DRY helpers for parsing v2 tables
fn parse_transforms_table_to_out(
    lua: &Lua,
    arr: mlua::Table,
    out: &mut Vec<f64>,
) -> mlua::Result<()> {
    let len = arr.raw_len();
    if len % 6 != 0 {
        return Err(mlua::Error::RuntimeError(format!(
            "ARG_ERROR: set_transforms stride mismatch (got={}, want=6)",
            len % 6
        )));
    }
    out.reserve(len);
    let mut i = 1;
    while i <= len {
        // id can be EntityId userdata or a number
        let v: mlua::Value = arr.raw_get(i)?;
        i += 1;
        let id_num = match v {
            mlua::Value::UserData(ud) => {
                if let Ok(ent) = ud.borrow::<EntityId>() {
                    ent.0 as f64
                } else {
                    return Err(mlua::Error::RuntimeError(
                        "ARG_ERROR: id must be EntityId or number".into(),
                    ));
                }
            }
            _ => f64::from_lua(v, lua)?,
        };
        out.push(id_num);
        // x,y,rot,sx,sy numbers
        let x: f64 = arr.raw_get(i)?;
        i += 1;
        out.push(x);
        let y: f64 = arr.raw_get(i)?;
        i += 1;
        out.push(y);
        let rot: f64 = arr.raw_get(i)?;
        i += 1;
        out.push(rot);
        let sx: f64 = arr.raw_get(i)?;
        i += 1;
        out.push(sx);
        let sy: f64 = arr.raw_get(i)?;
        i += 1;
        out.push(sy);
    }
    Ok(())
}

fn parse_sprites_table_to_out(
    _lua: &Lua,
    sprites: mlua::Table,
    out: &mut Vec<SpriteV2>,
) -> mlua::Result<()> {
    let len = sprites.raw_len();
    if len % 11 != 0 {
        return Err(mlua::Error::RuntimeError(format!(
            "ARG_ERROR: submit_sprites stride mismatch (got={}, want=11)",
            len % 11
        )));
    }
    out.reserve(len / 11);
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
        // remaining 9 numbers
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
        let z: f32 = sprites.raw_get(i)?;
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
            z,
        });
    }
    Ok(())
}

// FrameBuilder façade over typed buffers
pub struct FrameBuilder {
    t_key: RegistryKey,
    s_key: RegistryKey,
}

impl FrameBuilder {
    fn new(lua: &Lua, t: AnyUserData, s: AnyUserData) -> mlua::Result<Self> {
        let t_key = lua.create_registry_value(t)?;
        let s_key = lua.create_registry_value(s)?;
        Ok(Self { t_key, s_key })
    }
}

impl UserData for FrameBuilder {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Raw-units transform
        methods.add_method_mut("transform", |lua, this, (i, id_ud, x, y, rot, w, h): (usize, AnyUserData, f64, f64, f64, f64, f64)| {
            let t_ud: AnyUserData = lua.registry_value(&this.t_key)?;
            let tb = t_ud.borrow::<TransformBuffer>()?;
            let idx = i.checked_sub(1).ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
            let cap = *tb.cap.borrow(); if idx >= cap { return Err(mlua::Error::RuntimeError("index exceeds capacity".into())); }
            let entity_id = id_ud.borrow::<EntityId>()?.0 as f32;
            let off = idx * 6; let mut b = tb.buf.borrow_mut();
            b[off]=entity_id; b[off+1]=x as f32; b[off+2]=y as f32; b[off+3]=rot as f32; b[off+4]=w as f32; b[off+5]=h as f32;
            let mut l = tb.len.borrow_mut(); if i > *l { *l = i; }
            Ok(())
        });
        methods.add_method_mut(
            "sprite_tex",
            |lua,
             this,
             params: SpriteTexParams| {
                let (i, id_ud, tex_ud, u0, v0, u1, v1, r, g, b, a, z_opt) = params;
                let s_ud: AnyUserData = lua.registry_value(&this.s_key)?;
                let sb = s_ud.borrow::<SpriteBuffer>()?;
                let idx = i
                    .checked_sub(1)
                    .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
                let cap = *sb.cap.borrow();
                if idx >= cap {
                    return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
                }
                let entity_id = id_ud.borrow::<EntityId>()?.0;
                let texture_id = tex_ud.borrow::<TextureHandle>()?.0;
                let mut rows = sb.rows.borrow_mut();
                rows[idx] = SpriteData {
                    entity_id,
                    texture_id,
                    uv: [u0, v0, u1, v1],
                    color: [r, g, b, a],
                    z: z_opt.unwrap_or(0.0),
                };
                let mut l = sb.len.borrow_mut();
                if i > *l {
                    *l = i;
                }
                Ok(())
            },
        );
        methods.add_method_mut(
            "sprite_uv",
            |lua, this, (i, id_ud, u0, v0, u1, v1): (usize, AnyUserData, f32, f32, f32, f32)| {
                let s_ud: AnyUserData = lua.registry_value(&this.s_key)?;
                let sb = s_ud.borrow::<SpriteBuffer>()?;
                let idx = i
                    .checked_sub(1)
                    .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
                let cap = *sb.cap.borrow();
                if idx >= cap {
                    return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
                }
                let entity_id = id_ud.borrow::<EntityId>()?.0;
                let mut rows = sb.rows.borrow_mut();
                let row = &mut rows[idx];
                row.entity_id = entity_id;
                row.uv = [u0, v0, u1, v1];
                let mut l = sb.len.borrow_mut();
                if i > *l {
                    *l = i;
                }
                Ok(())
            },
        );
        methods.add_method_mut(
            "sprite_color",
            |lua, this, (i, r, g, b, a): (usize, f32, f32, f32, f32)| {
                let s_ud: AnyUserData = lua.registry_value(&this.s_key)?;
                let sb = s_ud.borrow::<SpriteBuffer>()?;
                let idx = i
                    .checked_sub(1)
                    .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
                let cap = *sb.cap.borrow();
                if idx >= cap {
                    return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
                }
                let mut rows = sb.rows.borrow_mut();
                let row = &mut rows[idx];
                row.color = [r, g, b, a];
                // length unchanged; no entity/tex set here
                Ok(())
            },
        );
        methods.add_method_mut(
            "sprite_named",
            |lua,
             this,
             (i, id_ud, atlas_ud, name, r, g, b, a, z_opt): (
                usize,
                AnyUserData,
                AnyUserData,
                String,
                f32,
                f32,
                f32,
                f32,
                Option<f32>,
            )| {
                let s_ud: AnyUserData = lua.registry_value(&this.s_key)?;
                let sb = s_ud.borrow::<SpriteBuffer>()?;
                let idx = i
                    .checked_sub(1)
                    .ok_or_else(|| mlua::Error::RuntimeError("index must be >= 1".into()))?;
                let cap = *sb.cap.borrow();
                if idx >= cap {
                    return Err(mlua::Error::RuntimeError("index exceeds capacity".into()));
                }
                let entity_id = id_ud.borrow::<EntityId>()?.0;
                let atlas = atlas_ud.borrow::<Atlas>()?;
                let uv = atlas.uv_map.get(&name).ok_or_else(|| {
                    mlua::Error::RuntimeError(format!("unknown atlas name: {}", name))
                })?;
                let mut rows = sb.rows.borrow_mut();
                rows[idx] = SpriteData {
                    entity_id,
                    texture_id: atlas.texture.0,
                    uv: [uv[0], uv[1], uv[2], uv[3]],
                    color: [r, g, b, a],
                    z: z_opt.unwrap_or(0.0),
                };
                let mut l = sb.len.borrow_mut();
                if i > *l {
                    *l = i;
                }
                Ok(())
            },
        );
        methods.add_method("commit", |lua, this, ()| {
            // engine.set_transforms(T); engine.submit_sprites(S)
            let globals = lua.globals();
            let engine_tbl: mlua::Table = globals.get("engine")?;
            let st: mlua::Function = engine_tbl.get("set_transforms")?;
            let ss: mlua::Function = engine_tbl.get("submit_sprites")?;
            let t_ud: AnyUserData = lua.registry_value(&this.t_key)?;
            let s_ud: AnyUserData = lua.registry_value(&this.s_key)?;
            st.call::<()>(t_ud)?;
            ss.call::<()>(s_ud)?;
            Ok(())
        });
    }
}

// (no tests in this module)
