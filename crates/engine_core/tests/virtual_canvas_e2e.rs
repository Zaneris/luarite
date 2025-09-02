use anyhow::Result;
use engine_core::{
    renderer::SpriteRenderer,
    state::{EngineState, SpriteData, VirtualResolution},
};
// Removed unused imports
use std::rc::Rc;
use std::cell::RefCell;

/// Wrapper around framebuffer data that handles coordinate system differences
///
/// The engine uses a coordinate system where y=0 is at the bottom (matching the orthographic projection),
/// but GPU framebuffers store data with y=0 at the top. This struct handles the conversion automatically.
struct FramebufferReader<'a> {
    data: &'a [u8],
    width: u32,
    height: u32,
}

impl<'a> FramebufferReader<'a> {
    fn new(data: &'a [u8], width: u32, height: u32) -> Self {
        Self {
            data,
            width,
            height,
        }
    }

    /// Get pixel in engine coordinates (y=0 at bottom, matching orthographic_lh projection)
    fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        // Convert from engine coordinates (y=0 bottom) to framebuffer coordinates (y=0 top)
        let framebuffer_y = self.height - 1 - y;
        let index = ((framebuffer_y * self.width + x) * 4) as usize;
        if index + 3 < self.data.len() {
            [
                self.data[index],
                self.data[index + 1],
                self.data[index + 2],
                self.data[index + 3],
            ]
        } else {
            [0, 0, 0, 0] // Return black for out-of-bounds access
        }
    }

}

/// Helper function to create a simple test texture (solid color) as PNG bytes
fn create_test_texture(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    use image::{codecs::png::PngEncoder, ImageBuffer, ImageEncoder, Rgba};

    // Create an image buffer with the solid color
    let img = ImageBuffer::from_fn(width, height, |_x, _y| Rgba(color));

    // Encode as PNG
    let mut png_data = Vec::new();
    {
        let encoder = PngEncoder::new(&mut png_data);
        encoder
            .write_image(img.as_raw(), width, height, image::ColorType::Rgba8.into())
            .expect("Failed to encode PNG");
    }

    png_data
}

/// Helper function to verify that a pixel is approximately the expected color (with some tolerance for GPU precision)
fn pixel_matches(actual: [u8; 4], expected: [u8; 4], tolerance: u8) -> bool {
    (actual[0] as i16 - expected[0] as i16).abs() <= tolerance as i16
        && (actual[1] as i16 - expected[1] as i16).abs() <= tolerance as i16
        && (actual[2] as i16 - expected[2] as i16).abs() <= tolerance as i16
        && (actual[3] as i16 - expected[3] as i16).abs() <= tolerance as i16
}

#[tokio::test]
async fn test_retro_mode_320x180_rendering() -> Result<()> {
    // Create headless renderer
    let mut renderer = SpriteRenderer::new_headless(640, 480).await?;

    // Create engine state with retro mode
    let mut state = EngineState::new();
    state.set_virtual_resolution(VirtualResolution::Retro320x180);
    state.set_clear_color(0.2, 0.3, 0.8, 1.0); // Blue background

    // Load a test texture - create a 32x32 red texture
    let red_texture = create_test_texture(32, 32, [255, 0, 0, 255]);
    state.insert_texture_with_id(1, "test_red", red_texture);

    // Create an entity with transform and sprite
    let entity_id = state.create_entity();
    let transforms = vec![
        entity_id as f64,
        160.0,
        90.0,
        0.0,
        64.0,
        64.0, // Center of 320x180 canvas, 64x64 size
    ];
    state.set_transforms(transforms)?;

    // Add sprite
    let sprite = SpriteData {
        entity_id,
        texture_id: 1,
        uv: [0.0, 0.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        z: 0.0,
    };
    state.submit_sprites(vec![sprite])?;

    // Render to virtual canvas
    let framebuffer_data = renderer.render_to_virtual_canvas(&state)?;

    // Verify virtual canvas dimensions (should be 320x180)
    let expected_size = 320 * 180 * 4; // RGBA
    assert_eq!(
        framebuffer_data.len(),
        expected_size,
        "Framebuffer should be 320x180 RGBA"
    );

    // Create framebuffer reader with engine coordinate system
    let fb = FramebufferReader::new(&framebuffer_data, 320, 180);

    // Check background color (should be blue)
    let bg_pixel = fb.get_pixel(10, 10);
    let expected_bg = [124, 149, 231, 255]; // sRGB gamma-corrected values
    assert!(
        pixel_matches(bg_pixel, expected_bg, 5),
        "Background should be blue, got {:?}, expected {:?}",
        bg_pixel,
        expected_bg
    );

    // Check that sprite is rendered at center (approximately)
    let sprite_pixel = fb.get_pixel(160, 90);
    let expected_sprite = [255, 0, 0, 255]; // Red texture
    assert!(
        pixel_matches(sprite_pixel, expected_sprite, 5),
        "Sprite should be red at center, got {:?}, expected {:?}",
        sprite_pixel,
        expected_sprite
    );

    // Check that sprite bounds are correct (should not extend beyond expected size)
    let outside_sprite = fb.get_pixel(160 + 40, 90);
    assert!(
        pixel_matches(outside_sprite, expected_bg, 5),
        "Area outside sprite should be background color"
    );

    Ok(())
}

#[tokio::test]
async fn test_hd_mode_1920x1080_rendering() -> Result<()> {
    // Create headless renderer
    let mut renderer = SpriteRenderer::new_headless(1920, 1080).await?;

    // Create engine state with HD mode
    let mut state = EngineState::new();
    state.set_virtual_resolution(VirtualResolution::Hd1920x1080);
    state.set_clear_color(0.1, 0.5, 0.1, 1.0); // Green background

    // Load a test texture - create a 64x64 blue texture
    let blue_texture = create_test_texture(64, 64, [0, 0, 255, 255]);
    state.insert_texture_with_id(1, "test_blue", blue_texture);

    // Create an entity with transform and sprite
    let entity_id = state.create_entity();
    let transforms = vec![
        entity_id as f64,
        960.0,
        540.0,
        0.0,
        128.0,
        128.0, // Center of 1920x1080 canvas, 128x128 size
    ];
    state.set_transforms(transforms)?;

    // Add sprite
    let sprite = SpriteData {
        entity_id,
        texture_id: 1,
        uv: [0.0, 0.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        z: 0.0,
    };
    state.submit_sprites(vec![sprite])?;

    // Render to virtual canvas
    let framebuffer_data = renderer.render_to_virtual_canvas(&state)?;

    // Verify virtual canvas dimensions (should be 1920x1080)
    let expected_size = 1920 * 1080 * 4; // RGBA
    assert_eq!(
        framebuffer_data.len(),
        expected_size,
        "Framebuffer should be 1920x1080 RGBA"
    );

    let fb = FramebufferReader::new(&framebuffer_data, 1920, 1080);

    // Check background color (should be green)
    let bg_pixel = fb.get_pixel(100, 100);
    let expected_bg = [89, 188, 89, 255]; // sRGB gamma-corrected values
    assert!(
        pixel_matches(bg_pixel, expected_bg, 5),
        "Background should be green, got {:?}, expected {:?}",
        bg_pixel,
        expected_bg
    );

    // Check that sprite is rendered at center
    let sprite_pixel = fb.get_pixel(960, 540);
    let expected_sprite = [0, 0, 255, 255]; // Blue texture
    assert!(
        pixel_matches(sprite_pixel, expected_sprite, 5),
        "Sprite should be blue at center, got {:?}, expected {:?}",
        sprite_pixel,
        expected_sprite
    );

    // Check that sprite bounds are correct
    let outside_sprite = fb.get_pixel(960 + 80, 540);
    assert!(
        pixel_matches(outside_sprite, expected_bg, 5),
        "Area outside sprite should be background color"
    );

    Ok(())
}

#[tokio::test]
async fn test_multiple_sprites_different_positions() -> Result<()> {
    let mut renderer = SpriteRenderer::new_headless(640, 360).await?;

    let mut state = EngineState::new();
    state.set_virtual_resolution(VirtualResolution::Retro320x180);
    state.set_clear_color(0.0, 0.0, 0.0, 1.0); // Black background

    // Create different colored textures
    let red_texture = create_test_texture(16, 16, [255, 0, 0, 255]);
    let green_texture = create_test_texture(16, 16, [0, 255, 0, 255]);
    let blue_texture = create_test_texture(16, 16, [0, 0, 255, 255]);

    state.insert_texture_with_id(1, "red", red_texture);
    state.insert_texture_with_id(2, "green", green_texture);
    state.insert_texture_with_id(3, "blue", blue_texture);

    // Create entities at different positions
    let entity1 = state.create_entity();
    let entity2 = state.create_entity();
    let entity3 = state.create_entity();

    let transforms = vec![
        entity1 as f64,
        80.0,
        45.0,
        0.0,
        32.0,
        32.0, // Top-left quadrant
        entity2 as f64,
        240.0,
        45.0,
        0.0,
        32.0,
        32.0, // Top-right quadrant
        entity3 as f64,
        160.0,
        135.0,
        0.0,
        32.0,
        32.0, // Bottom-center
    ];
    state.set_transforms(transforms)?;

    // Add sprites with different textures
    let sprites = vec![
        SpriteData {
            entity_id: entity1,
            texture_id: 1,
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            z: 0.0,
        },
        SpriteData {
            entity_id: entity2,
            texture_id: 2,
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            z: 0.0,
        },
        SpriteData {
            entity_id: entity3,
            texture_id: 3,
            uv: [0.0, 0.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            z: 0.0,
        },
    ];
    state.submit_sprites(sprites)?;

    // Render
    let framebuffer_data = renderer.render_to_virtual_canvas(&state)?;
    let fb = FramebufferReader::new(&framebuffer_data, 320, 180);

    // Check each sprite position
    let red_pixel = fb.get_pixel(80, 45);
    assert!(
        pixel_matches(red_pixel, [255, 0, 0, 255], 5),
        "Red sprite should be at (80,45)"
    );

    let green_pixel = fb.get_pixel(240, 45);
    assert!(
        pixel_matches(green_pixel, [0, 255, 0, 255], 5),
        "Green sprite should be at (240,45)"
    );

    let blue_pixel = fb.get_pixel(160, 135);
    assert!(
        pixel_matches(blue_pixel, [0, 0, 255, 255], 5),
        "Blue sprite should be at (160,135)"
    );

    Ok(())
}

#[tokio::test]
async fn test_virtual_resolution_switching() -> Result<()> {
    let mut renderer = SpriteRenderer::new_headless(640, 480).await?;

    // Test retro mode first
    let mut state = EngineState::new();
    state.set_virtual_resolution(VirtualResolution::Retro320x180);
    state.set_clear_color(1.0, 0.0, 0.0, 1.0); // Red

    let retro_data = renderer.render_to_virtual_canvas(&state)?;
    assert_eq!(
        retro_data.len(),
        320 * 180 * 4,
        "Retro mode should be 320x180"
    );

    // Switch to HD mode
    state.set_virtual_resolution(VirtualResolution::Hd1920x1080);
    state.set_clear_color(0.0, 1.0, 0.0, 1.0); // Green

    let hd_data = renderer.render_to_virtual_canvas(&state)?;
    assert_eq!(
        hd_data.len(),
        1920 * 1080 * 4,
        "HD mode should be 1920x1080"
    );

    // Verify colors are different (proving the switch worked)
    let retro_fb = FramebufferReader::new(&retro_data, 320, 180);
    let hd_fb = FramebufferReader::new(&hd_data, 1920, 1080);

    let retro_pixel = retro_fb.get_pixel(160, 90);
    let hd_pixel = hd_fb.get_pixel(960, 540);

    assert!(
        pixel_matches(retro_pixel, [255, 0, 0, 255], 5),
        "Retro should be red"
    );
    assert!(
        pixel_matches(hd_pixel, [0, 255, 0, 255], 5),
        "HD should be green"
    );

    Ok(())
}

#[tokio::test]
async fn test_sprite_rotation() -> Result<()> {
    let mut renderer = SpriteRenderer::new_headless(640, 360).await?;

    let mut state = EngineState::new();
    state.set_virtual_resolution(VirtualResolution::Retro320x180);
    state.set_clear_color(0.0, 0.0, 0.0, 1.0);

    // Create a rectangular texture to make rotation visible
    let texture_data = {
        use image::{codecs::png::PngEncoder, ImageBuffer, ImageEncoder, Rgba};

        let img = ImageBuffer::from_fn(16, 32, |x, _y| {
            if x < 8 {
                Rgba([255, 0, 0, 255]) // Red half
            } else {
                Rgba([0, 0, 255, 255]) // Blue half
            }
        });

        let mut png_data = Vec::new();
        let encoder = PngEncoder::new(&mut png_data);
        encoder
            .write_image(img.as_raw(), 16, 32, image::ColorType::Rgba8.into())
            .expect("Failed to encode PNG");
        png_data
    };

    state.insert_texture_with_id(1, "rect", texture_data);

    // Create entity with 45-degree rotation
    let entity_id = state.create_entity();
    let transforms = vec![
        entity_id as f64,
        160.0,
        90.0,
        std::f64::consts::PI / 4.0,
        32.0,
        64.0, // 45° rotation
    ];
    state.set_transforms(transforms)?;

    let sprite = SpriteData {
        entity_id,
        texture_id: 1,
        uv: [0.0, 0.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        z: 0.0,
    };
    state.submit_sprites(vec![sprite])?;

    let framebuffer_data = renderer.render_to_virtual_canvas(&state)?;
    let fb = FramebufferReader::new(&framebuffer_data, 320, 180);

    // With rotation, the sprite should extend into different areas
    // Just verify that we have non-background pixels in multiple positions around center
    let center_pixel = fb.get_pixel(160, 90);

    // The pixel should not be pure black (background) due to the rotated sprite
    let is_background = pixel_matches(center_pixel, [0, 0, 0, 255], 5);
    assert!(
        !is_background,
        "Rotated sprite should affect center pixel: got {:?}",
        center_pixel
    );

    Ok(())
}

#[tokio::test]
async fn test_sprite_alpha_blending() -> Result<()> {
    let mut renderer = SpriteRenderer::new_headless(320, 180).await?;

    let mut state = EngineState::new();
    state.set_virtual_resolution(VirtualResolution::Retro320x180);
    state.set_clear_color(0.5, 0.5, 0.5, 1.0); // Gray background

    // Create semi-transparent red texture
    let red_texture = create_test_texture(32, 32, [255, 0, 0, 128]);
    state.insert_texture_with_id(1, "semi_red", red_texture);

    let entity_id = state.create_entity();
    let transforms = vec![entity_id as f64, 160.0, 90.0, 0.0, 64.0, 64.0];
    state.set_transforms(transforms)?;

    let sprite = SpriteData {
        entity_id,
        texture_id: 1,
        uv: [0.0, 0.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 0.5], // Additional alpha
        z: 0.0,
    };
    state.submit_sprites(vec![sprite])?;

    let framebuffer_data = renderer.render_to_virtual_canvas(&state)?;
    let fb = FramebufferReader::new(&framebuffer_data, 320, 180);

    // Check that the pixel is blended (not pure red or pure gray)
    let blended_pixel = fb.get_pixel(160, 90);
    let pure_red = [255, 0, 0, 255];
    let gray_bg = [128, 128, 128, 255];

    // Should be neither pure red nor pure gray due to alpha blending
    assert!(
        !pixel_matches(blended_pixel, pure_red, 5),
        "Should not be pure red"
    );
    assert!(
        !pixel_matches(blended_pixel, gray_bg, 5),
        "Should not be pure gray"
    );

    // Should have some red component but not full intensity
    assert!(
        blended_pixel[0] > blended_pixel[1],
        "Should have more red than green"
    );
    assert!(
        blended_pixel[0] > blended_pixel[2],
        "Should have more red than blue"
    );
    assert!(blended_pixel[0] < 255, "Should not be full red intensity");

    Ok(())
}

/// E2E test for z-ordering: Sprites added in wrong order but correct z-values
/// Tests the full pipeline: Lua script → Engine API → SpriteData → Renderer → Virtual canvas
#[tokio::test]
async fn test_z_ordering_wrong_submission_order() -> Result<()> {
    use engine_scripting::{sandbox::LuaSandbox, api::{EngineApi, InputSnapshot}};
    
    // Create headless renderer
    let mut renderer = SpriteRenderer::new_headless(320, 180).await?;
    let mut engine_state = EngineState::new();
    engine_state.set_virtual_resolution(VirtualResolution::Retro320x180);
    engine_state.set_clear_color(0.0, 0.0, 0.0, 1.0);
    
    // Create shared state for capturing data from Lua callbacks
    let transforms_data: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let sprites_data: Rc<RefCell<Vec<SpriteData>>> = Rc::new(RefCell::new(Vec::new()));
    
    // Set up Lua sandbox and API
    let mut sandbox = LuaSandbox::new()?;
    let mut api = EngineApi::new();
    
    // Set up callbacks to capture sprite and transform data
    let transforms_capture = transforms_data.clone();
    let sprites_capture = sprites_data.clone();
    
    let set_transforms_cb = Rc::new(move |transforms: &[f64]| {
        transforms_capture.borrow_mut().clear();
        transforms_capture.borrow_mut().extend_from_slice(transforms);
    });
    
    let submit_sprites_cb = Rc::new(move |sprites: &[engine_scripting::api::SpriteV2]| {
        let mut sprites_vec = sprites_capture.borrow_mut();
        sprites_vec.clear();
        for s in sprites {
            sprites_vec.push(SpriteData {
                entity_id: s.entity_id,
                texture_id: s.texture_id,
                uv: [s.u0, s.v0, s.u1, s.v1],
                color: [s.r, s.g, s.b, s.a],
                z: s.z,
            });
        }
    });
    
    // Dummy callbacks for other engine functions
    let metrics_provider = Rc::new(|| (0.016, 60, 1)); // 60fps, 1 frame
    let load_texture_cb = Rc::new(|_path: String, _id: u32| {});
    let input_provider = Rc::new(|| InputSnapshot::default());
    let window_size_provider = Rc::new(|| (320u32, 180u32));
    let hud_printf_cb = Rc::new(|_msg: String| {});
    let set_clear_color_cb = Rc::new(|_r: f32, _g: f32, _b: f32, _a: f32| {});
    let set_render_mode_cb = Rc::new(|_mode: &'static str| {});
    
    // Install the API with callbacks
    api.setup_engine_namespace_with_sinks_and_metrics(
        &sandbox.lua(),
        set_transforms_cb,
        None, // No f32 transforms callback
        submit_sprites_cb,
        None, // No typed sprites callback
        metrics_provider,
        load_texture_cb,
        input_provider,
        window_size_provider,
        hud_printf_cb,
        set_clear_color_cb,
        set_render_mode_cb,
    )?;
    
    // Load and execute our z-ordering test script
    let script_content = r#"
-- Z-ordering test: Add sprites in WRONG order but with CORRECT z-values

assert(engine.api_version == 1)

-- Create entities in wrong order
local front_entity = engine.create_entity()  -- Will be z=2.0 (should appear on top)
local middle_entity = engine.create_entity() -- Will be z=1.0 (should appear in middle)  
local back_entity = engine.create_entity()   -- Will be z=0.0 (should appear behind)

local tex = engine.load_texture("dummy.png")
local T = engine.create_transform_buffer(3)
local S = engine.create_sprite_buffer(3)

function on_start()
  engine.set_clear_color(0.0, 0.0, 0.0, 1.0)
  engine.set_render_resolution("retro")
  
  -- Add sprites in WRONG ORDER but correct z-values
  S:set_tex(1, front_entity, tex)
  S:set_color(1, 0.0, 0.0, 1.0, 1.0)  -- BLUE - should appear on top despite being added first
  S:set_uv_rect(1, 0.0, 0.0, 1.0, 1.0)
  S:set_z(1, 2.0)  -- HIGHEST z-value
  
  S:set_tex(2, middle_entity, tex)  
  S:set_color(2, 0.0, 1.0, 0.0, 1.0)  -- GREEN - should appear in middle
  S:set_uv_rect(2, 0.0, 0.0, 1.0, 1.0)
  S:set_z(2, 1.0)  -- MIDDLE z-value
  
  S:set_tex(3, back_entity, tex)
  S:set_color(3, 1.0, 0.0, 0.0, 1.0)  -- RED - should appear behind despite being added last
  S:set_uv_rect(3, 0.0, 0.0, 1.0, 1.0)
  S:set_z(3, 0.0)  -- LOWEST z-value
end

function on_update(dt)
  -- Position all sprites to overlap completely at center
  local center_x, center_y = 160, 90
  local size = 80
  
  T:set(1, front_entity, center_x, center_y, 0, size, size)   -- Blue (z=2.0) 
  T:set(2, middle_entity, center_x, center_y, 0, size, size)  -- Green (z=1.0)
  T:set(3, back_entity, center_x, center_y, 0, size, size)    -- Red (z=0.0)
  
  engine.set_transforms(T)
  engine.submit_sprites(S)
end"#;
    
    sandbox.load_script(script_content, "z_order_test")?;
    
    // Execute on_start
    sandbox.call_function::<_, ()>("on_start", ())?;
    
    // Execute on_update to generate the frame
    sandbox.call_function::<_, ()>("on_update", 0.016)?; // 16ms frame time
    
    // Apply captured data to engine state
    if !transforms_data.borrow().is_empty() {
        engine_state.set_transforms(transforms_data.borrow().clone())?;
    }
    
    if !sprites_data.borrow().is_empty() {
        engine_state.submit_sprites(sprites_data.borrow().clone())?;
    }
    
    // Render to virtual canvas
    let framebuffer_data = renderer.render_to_virtual_canvas(&engine_state)?;
    let fb = FramebufferReader::new(&framebuffer_data, 320, 180);
    
    // Verify z-ordering: At center pixel (160, 90) we should see BLUE (front sprite)
    // despite it being added first in the wrong order
    let center_pixel = fb.get_pixel(160, 90);
    let expected_blue = [0, 0, 255, 255]; // Pure blue - the front sprite should be visible
    
    assert!(
        pixel_matches(center_pixel, expected_blue, 5),
        "Z-ordering failed! Expected blue (front sprite) at center, got {:?}. \
         Sprites were added in wrong order: FRONT first, MIDDLE second, BACK last, \
         but z-values should make BLUE appear on top (z=2.0 > z=1.0 > z=0.0)",
        center_pixel
    );
    
    // Also verify the background is black where no sprites are
    let bg_pixel = fb.get_pixel(50, 50); // Far from center, should be background
    let expected_bg = [0, 0, 0, 255]; // Black background
    assert!(
        pixel_matches(bg_pixel, expected_bg, 5),
        "Background should be black, got {:?}",
        bg_pixel
    );
    
    Ok(())
}

/// E2E test for z-ordering with partial overlaps and different textures
#[tokio::test] 
async fn test_z_ordering_partial_overlaps() -> Result<()> {
    use engine_scripting::{sandbox::LuaSandbox, api::{EngineApi, InputSnapshot}};
    
    let mut renderer = SpriteRenderer::new_headless(320, 180).await?;
    let mut engine_state = EngineState::new();
    engine_state.set_virtual_resolution(VirtualResolution::Retro320x180);
    engine_state.set_clear_color(0.0, 0.0, 0.0, 1.0);
    
    let transforms_data: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let sprites_data: Rc<RefCell<Vec<SpriteData>>> = Rc::new(RefCell::new(Vec::new()));
    
    let mut sandbox = LuaSandbox::new()?;
    let mut api = EngineApi::new();
    
    // Set up callbacks
    let transforms_capture = transforms_data.clone();
    let sprites_capture = sprites_data.clone();
    
    let set_transforms_cb = Rc::new(move |transforms: &[f64]| {
        transforms_capture.borrow_mut().clear();
        transforms_capture.borrow_mut().extend_from_slice(transforms);
    });
    
    let submit_sprites_cb = Rc::new(move |sprites: &[engine_scripting::api::SpriteV2]| {
        let mut sprites_vec = sprites_capture.borrow_mut();
        sprites_vec.clear();
        for s in sprites {
            sprites_vec.push(SpriteData {
                entity_id: s.entity_id,
                texture_id: s.texture_id, 
                uv: [s.u0, s.v0, s.u1, s.v1],
                color: [s.r, s.g, s.b, s.a],
                z: s.z,
            });
        }
    });
    
    let metrics_provider = Rc::new(|| (0.016, 60, 1)); // 60fps, 1 frame
    let load_texture_cb = Rc::new(|_path: String, _id: u32| {});
    let input_provider = Rc::new(|| InputSnapshot::default());
    let window_size_provider = Rc::new(|| (320u32, 180u32));
    let hud_printf_cb = Rc::new(|_msg: String| {});
    let set_clear_color_cb = Rc::new(|_r: f32, _g: f32, _b: f32, _a: f32| {});
    let set_render_mode_cb = Rc::new(|_mode: &'static str| {});
    
    api.setup_engine_namespace_with_sinks_and_metrics(
        &sandbox.lua(),
        set_transforms_cb,
        None,
        submit_sprites_cb,
        None,
        metrics_provider,
        load_texture_cb,
        input_provider,
        window_size_provider,
        hud_printf_cb,
        set_clear_color_cb,
        set_render_mode_cb,
    )?;
    
    // Complex partial overlap test script
    let script_content = r#"
assert(engine.api_version == 1)

-- Create entities in intentionally wrong order for submission
local high_z_entity = engine.create_entity()    -- z=5.0 (highest)
local low_z_entity = engine.create_entity()     -- z=1.0 (lowest)
local mid_z_entity = engine.create_entity()     -- z=3.0 (middle)

local tex = engine.load_texture("dummy.png")
local T = engine.create_transform_buffer(3)
local S = engine.create_sprite_buffer(3)

function on_start()
  engine.set_clear_color(0.0, 0.0, 0.0, 1.0)
  engine.set_render_resolution("retro")
  
  -- Submit in wrong order: HIGH first, LOW second, MID last
  -- But z-values will determine render order: LOW (z=1.0), MID (z=3.0), HIGH (z=5.0)
  
  S:set_tex(1, high_z_entity, tex)
  S:set_color(1, 0.0, 0.0, 1.0, 1.0)  -- BLUE - should be on top everywhere it appears
  S:set_uv_rect(1, 0.0, 0.0, 1.0, 1.0) 
  S:set_z(1, 5.0)  -- HIGHEST z-value
  
  S:set_tex(2, low_z_entity, tex)
  S:set_color(2, 1.0, 0.0, 0.0, 1.0)  -- RED - should be behind everything
  S:set_uv_rect(2, 0.0, 0.0, 1.0, 1.0)
  S:set_z(2, 1.0)  -- LOWEST z-value
  
  S:set_tex(3, mid_z_entity, tex)
  S:set_color(3, 0.0, 1.0, 0.0, 1.0)  -- GREEN - should be in middle 
  S:set_uv_rect(3, 0.0, 0.0, 1.0, 1.0)
  S:set_z(3, 3.0)  -- MIDDLE z-value
end

function on_update(dt)
  -- Create predictable partial overlaps with three-way overlap region:
  -- RED at (140, 90), GREEN at (160, 90), BLUE at (180, 90)
  -- Each sprite is 60x60, creating overlaps:
  -- Red spans 110-170, Green spans 130-190, Blue spans 150-210
  -- Three-way overlap: x=150-170 (should show BLUE z=5.0)
  
  local size = 60
  
  T:set(1, high_z_entity, 180, 90, 0, size, size)  -- Blue (z=5.0) - rightmost
  T:set(2, low_z_entity, 140, 90, 0, size, size)   -- Red (z=1.0) - leftmost
  T:set(3, mid_z_entity, 160, 90, 0, size, size)   -- Green (z=3.0) - center
  
  engine.set_transforms(T)
  engine.submit_sprites(S)
end"#;
    
    sandbox.load_script(script_content, "partial_overlap_test")?;
    sandbox.call_function::<_, ()>("on_start", ())?;
    sandbox.call_function::<_, ()>("on_update", 0.016)?;
    
    // Apply data to engine state
    if !transforms_data.borrow().is_empty() {
        engine_state.set_transforms(transforms_data.borrow().clone())?;
    }
    if !sprites_data.borrow().is_empty() {
        engine_state.submit_sprites(sprites_data.borrow().clone())?;
    }
    
    let framebuffer_data = renderer.render_to_virtual_canvas(&engine_state)?;
    let fb = FramebufferReader::new(&framebuffer_data, 320, 180);
    
    // Test specific overlap regions with new positioning:
    // Red at (140,90) spans 110-170, Green at (160,90) spans 130-190, Blue at (180,90) spans 150-210
    // - Left area (120, 90): Only red sprite, should show red
    // - Center-left overlap (140, 90): Red + Green overlap, GREEN should win (z=3.0 > z=1.0) 
    // - Center overlap (160, 90): All three overlap, BLUE should win (z=5.0 highest)
    // - Center-right overlap (180, 90): Green + Blue overlap, BLUE should win (z=5.0 > z=3.0)
    // - Right area (200, 90): Only blue sprite, should show blue
    
    let red_only = fb.get_pixel(120, 90);
    assert!(
        pixel_matches(red_only, [255, 0, 0, 255], 5),
        "Red-only area should show red, got {:?}",
        red_only
    );
    
    let red_green_overlap = fb.get_pixel(140, 90);
    assert!(
        pixel_matches(red_green_overlap, [0, 255, 0, 255], 5),
        "Red+Green overlap should show GREEN (z=3.0 > z=1.0), got {:?}",
        red_green_overlap
    );
    
    let all_three_overlap = fb.get_pixel(160, 90);
    assert!(
        pixel_matches(all_three_overlap, [0, 0, 255, 255], 5),
        "All three overlap should show BLUE (z=5.0 highest), got {:?}",
        all_three_overlap
    );
    
    let green_blue_overlap = fb.get_pixel(180, 90);
    assert!(
        pixel_matches(green_blue_overlap, [0, 0, 255, 255], 5),
        "Green+Blue overlap should show BLUE (z=5.0 > z=3.0), got {:?}",
        green_blue_overlap
    );
    
    let blue_only = fb.get_pixel(200, 90);
    assert!(
        pixel_matches(blue_only, [0, 0, 255, 255], 5),
        "Blue-only area should show blue, got {:?}",
        blue_only
    );
    
    Ok(())
}
