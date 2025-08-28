use anyhow::Result;
use std::collections::HashMap;

/// Sprite data for v2 flat array format (engine-native representation)
#[derive(Debug, Clone)]
pub struct SpriteData {
    pub entity_id: u32,
    pub texture_id: u32,
    pub uv: [f32; 4],    // u0, v0, u1, v1
    pub color: [f32; 4], // r, g, b, a
}

/// Central engine state that owns all game resources
/// This is the single source of truth for all game data
#[derive(Debug)]
pub struct EngineState {
    // Transform data (v2 flat array format)
    transform_buffer: Vec<f64>, // stride=6: [id, x, y, rot, sx, sy, ...]

    // Sprite data (engine-native representation)
    sprites: Vec<SpriteData>,

    // Texture storage
    textures: HashMap<u32, Vec<u8>>,     // texture_id -> raw bytes
    texture_names: HashMap<u32, String>, // texture_id -> name for debugging

    // Entity management
    next_entity_id: u32,
    next_texture_id: u32,

    // Time management
    fixed_time: f64,

    // Performance tracking
    ffi_calls_this_frame: u32,
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            transform_buffer: Vec::with_capacity(10000 * 6), // Pre-allocate for max entities
            sprites: Vec::with_capacity(10000),              // Pre-allocate for max entities
            textures: HashMap::new(),
            texture_names: HashMap::new(),
            next_entity_id: 1,
            next_texture_id: 1,
            fixed_time: 0.0,
            ffi_calls_this_frame: 0,
        }
    }

    // Entity Management
    pub fn create_entity(&mut self) -> u32 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        tracing::debug!("Created entity {}", id);
        id
    }

    // Texture Management
    pub fn load_texture(&mut self, path: &str, bytes: Vec<u8>) -> Result<u32> {
        let id = self.next_texture_id;
        self.next_texture_id += 1;

        self.textures.insert(id, bytes);
        self.texture_names.insert(id, path.to_string());

        tracing::info!("Loaded texture '{}' with ID {}", path, id);
        Ok(id)
    }

    pub fn get_texture(&self, texture_id: u32) -> Option<&Vec<u8>> {
        self.textures.get(&texture_id)
    }

    pub fn get_texture_name(&self, texture_id: u32) -> Option<&str> {
        self.texture_names.get(&texture_id).map(|s| s.as_str())
    }

    // Transform Management (v2 flat array format)
    pub fn set_transforms(&mut self, transforms: Vec<f64>) -> Result<()> {
        if transforms.len() % 6 != 0 {
            return Err(anyhow::anyhow!(
                "ARG_ERROR: set_transforms stride mismatch (got={}, want=6)", 
                transforms.len() % 6
            ));
        }

        // Clamp to max entities budget to avoid abuse
        let max_entities = 10_000usize;
        let elems = transforms.len().min(max_entities * 6);
        self.transform_buffer.clear();
        self.transform_buffer.extend_from_slice(&transforms[..elems]);
        self.ffi_calls_this_frame += 1;

        tracing::debug!("Set {} transforms", transforms.len() / 6);
        Ok(())
    }

    pub fn get_transforms(&self) -> &[f64] {
        &self.transform_buffer
    }

    // Sprite Management (engine-native format)
    pub fn submit_sprites(&mut self, sprites: Vec<SpriteData>) -> Result<()> {
        self.sprites.clear();
        self.sprites.extend(sprites);
        self.ffi_calls_this_frame += 1;

        tracing::debug!("Submitted {} sprites", self.sprites.len());
        Ok(())
    }

    pub fn get_sprites(&self) -> &[SpriteData] {
        &self.sprites
    }

    // Time Management
    pub fn update_time(&mut self, dt: f64) {
        self.fixed_time += dt;
    }

    pub fn get_time(&self) -> f64 {
        self.fixed_time
    }

    // Note: Persistence will be handled in the engine_scripting layer
    // This keeps engine_core free of Lua dependencies

    // Performance Metrics
    pub fn get_ffi_calls_this_frame(&self) -> u32 {
        self.ffi_calls_this_frame
    }

    pub fn reset_frame_counters(&mut self) {
        self.ffi_calls_this_frame = 0;
    }

    // Validation against plan requirements
    pub fn validate_performance_budgets(&self) -> Vec<String> {
        let mut violations = Vec::new();

        // Check FFI calls per frame <= 3 (per plan)
        if self.ffi_calls_this_frame > 3 {
            violations.push(format!(
                "ffi_calls_per_frame ({}) exceeds budget of 3",
                self.ffi_calls_this_frame
            ));
        }

        violations
    }
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}
