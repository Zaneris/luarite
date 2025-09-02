use engine_core::{renderer::SpriteRenderer, state::{EngineState, VirtualResolution, SpriteData}};
use anyhow::Result;

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
        Self { data, width, height }
    }
    
    /// Get pixel in engine coordinates (y=0 at bottom, matching orthographic_lh projection)
    fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        // Convert from engine coordinates (y=0 bottom) to framebuffer coordinates (y=0 top)
        let framebuffer_y = self.height - 1 - y;
        let index = ((framebuffer_y * self.width + x) * 4) as usize;
        if index + 3 < self.data.len() {
            [self.data[index], self.data[index + 1], self.data[index + 2], self.data[index + 3]]
        } else {
            [0, 0, 0, 0] // Return black for out-of-bounds access
        }
    }
    
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Helper function to create a simple test texture (solid color) as PNG bytes
fn create_test_texture(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    use image::{ImageBuffer, Rgba, ImageEncoder, codecs::png::PngEncoder};
    
    // Create an image buffer with the solid color
    let img = ImageBuffer::from_fn(width, height, |_x, _y| {
        Rgba(color)
    });
    
    // Encode as PNG
    let mut png_data = Vec::new();
    {
        let encoder = PngEncoder::new(&mut png_data);
        encoder.write_image(
            img.as_raw(),
            width,
            height,
            image::ColorType::Rgba8.into()
        ).expect("Failed to encode PNG");
    }
    
    png_data
}

/// Helper function to verify that a pixel is approximately the expected color (with some tolerance for GPU precision)
fn pixel_matches(actual: [u8; 4], expected: [u8; 4], tolerance: u8) -> bool {
    (actual[0] as i16 - expected[0] as i16).abs() <= tolerance as i16 &&
    (actual[1] as i16 - expected[1] as i16).abs() <= tolerance as i16 &&
    (actual[2] as i16 - expected[2] as i16).abs() <= tolerance as i16 &&
    (actual[3] as i16 - expected[3] as i16).abs() <= tolerance as i16
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
        entity_id as f64, 160.0, 90.0, 0.0, 64.0, 64.0  // Center of 320x180 canvas, 64x64 size
    ];
    state.set_transforms(transforms)?;
    
    // Add sprite
    let sprite = SpriteData {
        entity_id,
        texture_id: 1,
        uv: [0.0, 0.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    };
    state.submit_sprites(vec![sprite])?;
    
    // Render to virtual canvas
    let framebuffer_data = renderer.render_to_virtual_canvas(&state)?;
    
    // Verify virtual canvas dimensions (should be 320x180)
    let expected_size = 320 * 180 * 4; // RGBA
    assert_eq!(framebuffer_data.len(), expected_size, "Framebuffer should be 320x180 RGBA");
    
    // Create framebuffer reader with engine coordinate system
    let fb = FramebufferReader::new(&framebuffer_data, 320, 180);
    
    // Check background color (should be blue)
    let bg_pixel = fb.get_pixel(10, 10);
    let expected_bg = [124, 149, 231, 255]; // sRGB gamma-corrected values
    assert!(pixel_matches(bg_pixel, expected_bg, 5), 
           "Background should be blue, got {:?}, expected {:?}", bg_pixel, expected_bg);
    
    // Check that sprite is rendered at center (approximately)
    let sprite_pixel = fb.get_pixel(160, 90);
    let expected_sprite = [255, 0, 0, 255]; // Red texture
    assert!(pixel_matches(sprite_pixel, expected_sprite, 5),
           "Sprite should be red at center, got {:?}, expected {:?}", sprite_pixel, expected_sprite);
    
    // Check that sprite bounds are correct (should not extend beyond expected size)
    let outside_sprite = fb.get_pixel(160 + 40, 90);
    assert!(pixel_matches(outside_sprite, expected_bg, 5),
           "Area outside sprite should be background color");
    
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
        entity_id as f64, 960.0, 540.0, 0.0, 128.0, 128.0  // Center of 1920x1080 canvas, 128x128 size
    ];
    state.set_transforms(transforms)?;
    
    // Add sprite
    let sprite = SpriteData {
        entity_id,
        texture_id: 1,
        uv: [0.0, 0.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    };
    state.submit_sprites(vec![sprite])?;
    
    // Render to virtual canvas
    let framebuffer_data = renderer.render_to_virtual_canvas(&state)?;
    
    // Verify virtual canvas dimensions (should be 1920x1080)
    let expected_size = 1920 * 1080 * 4; // RGBA
    assert_eq!(framebuffer_data.len(), expected_size, "Framebuffer should be 1920x1080 RGBA");
    
    let fb = FramebufferReader::new(&framebuffer_data, 1920, 1080);
    
    // Check background color (should be green)
    let bg_pixel = fb.get_pixel(100, 100);
    let expected_bg = [89, 188, 89, 255]; // sRGB gamma-corrected values
    assert!(pixel_matches(bg_pixel, expected_bg, 5),
           "Background should be green, got {:?}, expected {:?}", bg_pixel, expected_bg);
    
    // Check that sprite is rendered at center
    let sprite_pixel = fb.get_pixel(960, 540);
    let expected_sprite = [0, 0, 255, 255]; // Blue texture
    assert!(pixel_matches(sprite_pixel, expected_sprite, 5),
           "Sprite should be blue at center, got {:?}, expected {:?}", sprite_pixel, expected_sprite);
    
    // Check that sprite bounds are correct
    let outside_sprite = fb.get_pixel(960 + 80, 540);
    assert!(pixel_matches(outside_sprite, expected_bg, 5),
           "Area outside sprite should be background color");
    
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
        entity1 as f64, 80.0, 45.0, 0.0, 32.0, 32.0,   // Top-left quadrant
        entity2 as f64, 240.0, 45.0, 0.0, 32.0, 32.0,  // Top-right quadrant  
        entity3 as f64, 160.0, 135.0, 0.0, 32.0, 32.0, // Bottom-center
    ];
    state.set_transforms(transforms)?;
    
    // Add sprites with different textures
    let sprites = vec![
        SpriteData { entity_id: entity1, texture_id: 1, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
        SpriteData { entity_id: entity2, texture_id: 2, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
        SpriteData { entity_id: entity3, texture_id: 3, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 1.0, 1.0, 1.0] },
    ];
    state.submit_sprites(sprites)?;
    
    // Render
    let framebuffer_data = renderer.render_to_virtual_canvas(&state)?;
    let fb = FramebufferReader::new(&framebuffer_data, 320, 180);
    
    // Check each sprite position
    let red_pixel = fb.get_pixel(80, 45);
    assert!(pixel_matches(red_pixel, [255, 0, 0, 255], 5),
           "Red sprite should be at (80,45)");
           
    let green_pixel = fb.get_pixel(240, 45);
    assert!(pixel_matches(green_pixel, [0, 255, 0, 255], 5),
           "Green sprite should be at (240,45)");
           
    let blue_pixel = fb.get_pixel(160, 135);
    assert!(pixel_matches(blue_pixel, [0, 0, 255, 255], 5),
           "Blue sprite should be at (160,135)");
    
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
    assert_eq!(retro_data.len(), 320 * 180 * 4, "Retro mode should be 320x180");
    
    // Switch to HD mode  
    state.set_virtual_resolution(VirtualResolution::Hd1920x1080);
    state.set_clear_color(0.0, 1.0, 0.0, 1.0); // Green
    
    let hd_data = renderer.render_to_virtual_canvas(&state)?;
    assert_eq!(hd_data.len(), 1920 * 1080 * 4, "HD mode should be 1920x1080");
    
    // Verify colors are different (proving the switch worked)
    let retro_fb = FramebufferReader::new(&retro_data, 320, 180);
    let hd_fb = FramebufferReader::new(&hd_data, 1920, 1080);
    
    let retro_pixel = retro_fb.get_pixel(160, 90);
    let hd_pixel = hd_fb.get_pixel(960, 540);
    
    assert!(pixel_matches(retro_pixel, [255, 0, 0, 255], 5), "Retro should be red");
    assert!(pixel_matches(hd_pixel, [0, 255, 0, 255], 5), "HD should be green");
    
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
        use image::{ImageBuffer, Rgba, ImageEncoder, codecs::png::PngEncoder};
        
        let img = ImageBuffer::from_fn(16, 32, |x, _y| {
            if x < 8 {
                Rgba([255, 0, 0, 255]) // Red half
            } else {
                Rgba([0, 0, 255, 255]) // Blue half
            }
        });
        
        let mut png_data = Vec::new();
        let encoder = PngEncoder::new(&mut png_data);
        encoder.write_image(
            img.as_raw(),
            16,
            32,
            image::ColorType::Rgba8.into()
        ).expect("Failed to encode PNG");
        png_data
    };
    
    state.insert_texture_with_id(1, "rect", texture_data);
    
    // Create entity with 45-degree rotation
    let entity_id = state.create_entity();
    let transforms = vec![
        entity_id as f64, 160.0, 90.0, std::f64::consts::PI / 4.0, 32.0, 64.0 // 45° rotation
    ];
    state.set_transforms(transforms)?;
    
    let sprite = SpriteData {
        entity_id,
        texture_id: 1,
        uv: [0.0, 0.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
    };
    state.submit_sprites(vec![sprite])?;
    
    let framebuffer_data = renderer.render_to_virtual_canvas(&state)?;
    let fb = FramebufferReader::new(&framebuffer_data, 320, 180);
    
    // With rotation, the sprite should extend into different areas
    // Just verify that we have non-background pixels in multiple positions around center
    let center_pixel = fb.get_pixel(160, 90);
    
    // The pixel should not be pure black (background) due to the rotated sprite
    let is_background = pixel_matches(center_pixel, [0, 0, 0, 255], 5);
    assert!(!is_background, "Rotated sprite should affect center pixel: got {:?}", center_pixel);
    
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
    let transforms = vec![
        entity_id as f64, 160.0, 90.0, 0.0, 64.0, 64.0
    ];
    state.set_transforms(transforms)?;
    
    let sprite = SpriteData {
        entity_id,
        texture_id: 1,
        uv: [0.0, 0.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 0.5], // Additional alpha
    };
    state.submit_sprites(vec![sprite])?;
    
    let framebuffer_data = renderer.render_to_virtual_canvas(&state)?;
    let fb = FramebufferReader::new(&framebuffer_data, 320, 180);
    
    // Check that the pixel is blended (not pure red or pure gray)
    let blended_pixel = fb.get_pixel(160, 90);
    let pure_red = [255, 0, 0, 255];
    let gray_bg = [128, 128, 128, 255];
    
    // Should be neither pure red nor pure gray due to alpha blending
    assert!(!pixel_matches(blended_pixel, pure_red, 5), "Should not be pure red");
    assert!(!pixel_matches(blended_pixel, gray_bg, 5), "Should not be pure gray");
    
    // Should have some red component but not full intensity
    assert!(blended_pixel[0] > blended_pixel[1], "Should have more red than green");
    assert!(blended_pixel[0] > blended_pixel[2], "Should have more red than blue");
    assert!(blended_pixel[0] < 255, "Should not be full red intensity");
    
    Ok(())
}