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
    transform_buffer: Vec<f32>, // stride=6: [id, x, y, rot, sx, sy, ...]

    // Sprite data (engine-native representation)
    sprites_front: Vec<SpriteData>,
    sprites_back: Vec<SpriteData>,

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

    // Window info
    window_width: u32,
    window_height: u32,
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            transform_buffer: Vec::with_capacity(10000 * 6), // Pre-allocate for max entities
            sprites_front: Vec::with_capacity(10000),        // Pre-allocate for max entities
            sprites_back: Vec::with_capacity(10000),
            textures: HashMap::new(),
            texture_names: HashMap::new(),
            next_entity_id: 1,
            next_texture_id: 1,
            fixed_time: 0.0,
            ffi_calls_this_frame: 0,
            window_width: 1024,
            window_height: 768,
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

    pub fn insert_texture_with_id(&mut self, id: u32, path: &str, bytes: Vec<u8>) {
        self.textures.insert(id, bytes);
        self.texture_names.insert(id, path.to_string());
        if id >= self.next_texture_id {
            self.next_texture_id = id + 1;
        }
        tracing::info!("Inserted texture '{}' with provided ID {}", path, id);
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
        // Convert to f32 for internal storage
        self.transform_buffer
            .extend(transforms[..elems].iter().map(|v| *v as f32));
        self.ffi_calls_this_frame += 1;

        tracing::debug!("Set {} transforms", transforms.len() / 6);
        Ok(())
    }

    pub fn set_transforms_from_slice(&mut self, transforms: &[f64]) -> Result<()> {
        if transforms.len() % 6 != 0 {
            return Err(anyhow::anyhow!(
                "ARG_ERROR: set_transforms stride mismatch (got={}, want=6)",
                transforms.len() % 6
            ));
        }
        let max_entities = 10_000usize;
        let elems = transforms.len().min(max_entities * 6);
        self.transform_buffer.clear();
        self.transform_buffer
            .extend(transforms[..elems].iter().map(|v| *v as f32));
        self.ffi_calls_this_frame += 1;
        tracing::debug!("Set {} transforms", elems / 6);
        Ok(())
    }

    pub fn set_transforms_from_f32_slice(&mut self, transforms: &[f32]) -> Result<()> {
        if transforms.len() % 6 != 0 {
            return Err(anyhow::anyhow!(
                "ARG_ERROR: set_transforms stride mismatch (got={}, want=6)",
                transforms.len() % 6
            ));
        }
        let max_entities = 10_000usize;
        let elems = transforms.len().min(max_entities * 6);
        self.transform_buffer.clear();
        self.transform_buffer.extend_from_slice(&transforms[..elems]);
        self.ffi_calls_this_frame += 1;
        tracing::debug!("Set {} transforms (f32)", elems / 6);
        Ok(())
    }

    pub fn get_transforms(&self) -> &[f32] {
        &self.transform_buffer
    }

    // Sprite Management (engine-native format)
    pub fn submit_sprites(&mut self, sprites: Vec<SpriteData>) -> Result<()> {
        self.sprites_front.clear();
        let max = 10_000usize;
        if sprites.len() > max {
            tracing::warn!("submit_sprites clamped from {} to {}", sprites.len(), max);
        }
        let take = sprites.into_iter().take(max);
        self.sprites_front.extend(take);
        self.ffi_calls_this_frame += 1;

        tracing::debug!("Submitted {} sprites", self.sprites_front.len());
        Ok(())
    }

    pub fn append_sprites(&mut self, sprites: &mut Vec<SpriteData>) -> Result<()> {
        self.sprites_front.clear();
        let max = 10_000usize;
        if sprites.len() > max {
            tracing::warn!("append_sprites clamped from {} to {}", sprites.len(), max);
            sprites.truncate(max);
        }
        self.sprites_front.append(sprites);
        self.ffi_calls_this_frame += 1;
        tracing::debug!("Submitted {} sprites", self.sprites_front.len());
        Ok(())
    }

    // Zero-copy: swap Lua-owned vec into back buffer, taking only `len` rows
    pub fn swap_typed_sprites_into_back(&mut self, script_vec: &mut Vec<SpriteData>, len: usize) {
        use std::cmp::min;
        std::mem::swap(&mut self.sprites_back, script_vec);
        let take = min(self.sprites_back.len(), len);
        self.sprites_back.truncate(take);
        self.ffi_calls_this_frame += 1;
        tracing::debug!("Swapped sprites into back buffer ({} sprites)", take);
    }

    // Promote back buffer (freshly swapped) into front buffer visible to the renderer (zero-copy move)
    pub fn promote_sprites_back_to_front(&mut self) {
        if !self.sprites_back.is_empty() {
            std::mem::swap(&mut self.sprites_front, &mut self.sprites_back);
        }
    }

    // Restore Lua-side vec by swapping whatever remains in back buffer back into the script vec,
    // then resize script vec to its capacity.
    pub fn restore_lua_sprite_vec(&mut self, script_vec: &mut Vec<SpriteData>, cap: usize) {
        std::mem::swap(&mut self.sprites_back, script_vec);
        if script_vec.len() < cap {
            script_vec.resize(cap, SpriteData { entity_id: 0, texture_id: 0, uv: [0.0;4], color: [0.0;4] });
        }
    }

    pub fn get_sprites(&self) -> &[SpriteData] {
        &self.sprites_front
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

    // Window size management
    pub fn set_window_size(&mut self, w: u32, h: u32) {
        self.window_width = w;
        self.window_height = h;
    }

    pub fn window_size(&self) -> (u32, u32) {
        (self.window_width, self.window_height)
    }

    // Determinism: compute a stable hash of the transform buffer
    pub fn compute_transform_hash(&self) -> u64 {
        // FNV-1a 64-bit over f64 bit patterns
        let mut hash: u64 = 0xcbf29ce484222325;
        let prime: u64 = 0x100000001b3;
        for v in &self.transform_buffer {
            let bits = v.to_bits();
            let b = bits.to_le_bytes();
            for byte in b {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(prime);
            }
        }
        hash
    }

    // Zero-copy swap of the transform buffer with a script-owned buffer, taking only `elems` items
    pub fn swap_transform_buffer_with_len(&mut self, script_buf: &mut Vec<f32>, elems: usize) {
        use std::cmp::min;
        std::mem::swap(&mut self.transform_buffer, script_buf);
        let take = min(self.transform_buffer.len(), elems);
        self.transform_buffer.truncate(take);
        self.ffi_calls_this_frame += 1;
        tracing::debug!("Swapped transform buffer ({} elems)", take / 6);
    }
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}
