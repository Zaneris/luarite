use anyhow::Result;
use engine_core::{
    renderer::SpriteRenderer,
    state::{EngineState, SpriteData, VirtualResolution},
};
use image::ImageEncoder;
use std::cell::RefCell;
use std::rc::Rc;



/// Wrapper around framebuffer data that handles coordinate system differences
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
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0]; // Out of bounds
        }
        // Convert from engine coordinates (y=0 bottom) to framebuffer coordinates (y=0 top)
        let framebuffer_y = self.height - 1 - y;
        let index = ((framebuffer_y * self.width + x) * 4) as usize;
        [
            self.data[index],
            self.data[index + 1],
            self.data[index + 2],
            self.data[index + 3],
        ]
    }
}

/// Helper function to create a simple test texture (solid color) as PNG bytes
fn create_test_texture(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    use image::{codecs::png::PngEncoder, ImageBuffer, ImageEncoder, Rgba};
    let img = ImageBuffer::from_fn(width, height, |_x, _y| Rgba(color));
    let mut png_data = Vec::new();
    PngEncoder::new(&mut png_data)
        .write_image(img.as_raw(), width, height, image::ColorType::Rgba8.into())
        .expect("Failed to encode PNG");
    png_data
}

/// Helper function to verify that a pixel is approximately the expected color
fn pixel_matches(actual: [u8; 4], expected: [u8; 4], tolerance: u8) -> bool {
    (actual[0] as i16 - expected[0] as i16).abs() <= tolerance as i16
        && (actual[1] as i16 - expected[1] as i16).abs() <= tolerance as i16
        && (actual[2] as i16 - expected[2] as i16).abs() <= tolerance as i16
        && (actual[3] as i16 - expected[3] as i16).abs() <= tolerance as i16
}

/// Test harness that executes Lua scripts and captures results for verification.
struct E2ETestHarness {
    textures: Rc<RefCell<Vec<(u32, String, Vec<u8>)>>>,
}

impl E2ETestHarness {
    fn new() -> Self {
        Self {
            textures: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Add a texture that can be loaded by the script.
    fn add_texture(&self, id: u32, name: &str, data: Vec<u8>) {
        self.textures.borrow_mut().push((id, name.to_string(), data));
    }

    /// Execute a Lua script and render the result to the virtual canvas.
    async fn execute_script(&self, script_content: &str, script_name: &str) -> Result<Vec<u8>> {
        use engine_scripting::{api::EngineApi, sandbox::LuaSandbox};

        let sandbox = LuaSandbox::new()?;
        let api = EngineApi::new();

        // We need to capture the final state submitted by the sugar API
        let transforms_capture = Rc::new(RefCell::new(Vec::new()));
        let sprites_capture = Rc::new(RefCell::new(Vec::new()));
        let clear_color_capture = Rc::new(RefCell::new(None));
        let render_mode_capture = Rc::new(RefCell::new(None));

        let callbacks = engine_scripting::api::EngineCallbacks {
            set_transforms_cb: {
                let transforms_shared = transforms_capture.clone();
                Rc::new(move |transforms: &[f64]| {
                    *transforms_shared.borrow_mut() = transforms.to_vec();
                })
            },
            submit_sprites_cb: {
                let sprites_shared = sprites_capture.clone();
                Rc::new(move |sprites: &[engine_scripting::api::SpriteV2]| {
                    *sprites_shared.borrow_mut() = sprites.to_vec();
                })
            },
            set_clear_color_cb: {
                let clear_color_shared = clear_color_capture.clone();
                Rc::new(move |r, g, b, a| {
                    *clear_color_shared.borrow_mut() = Some((r, g, b, a));
                })
            },
            set_render_mode_cb: {
                let render_mode_shared = render_mode_capture.clone();
                Rc::new(move |mode: &'static str| {
                    *render_mode_shared.borrow_mut() = Some(mode.to_string());
                })
            },
            // These are not used by the sugar API in the same way, so they can be dummy.
            set_transforms_f32_cb: None,
            submit_sprites_typed_cb: None,
            metrics_provider: Rc::new(|| (0.016, 60, 1)),
            load_texture_cb: Rc::new(|_path, _id| {}),
            input_provider: Rc::new(Default::default),
            window_size_provider: Rc::new(|| (320, 180)),
            hud_printf_cb: Rc::new(|_msg| {}),
        };

        api.setup_engine_namespace_with_sinks_and_metrics(sandbox.lua(), callbacks)?;

        sandbox.load_script(script_content, script_name)?;
        sandbox.call_function::<_, ()>("on_start", ())?;
        sandbox.call_function::<_, ()>("on_update", 0.016)?;

        // Now, render the state that was captured from the callbacks.
        let render_mode = render_mode_capture.borrow().clone().unwrap_or_else(|| "retro".to_string());
        let (virtual_res, window_size) = match render_mode.as_str() {
            "hd" => (VirtualResolution::Hd1920x1080, (1920, 1080)),
            _ => (VirtualResolution::Retro320x180, (640, 480)),
        };

        let mut renderer = SpriteRenderer::new_headless(window_size.0, window_size.1).await?;
        let mut engine_state = EngineState::new();
        engine_state.set_virtual_resolution(virtual_res);

        if let Some((r, g, b, a)) = *clear_color_capture.borrow() {
            engine_state.set_clear_color(r, g, b, a);
        }

        let transforms = transforms_capture.borrow().clone();
        if !transforms.is_empty() {
            engine_state.set_transforms(transforms)?;
        }

        let sprites = sprites_capture.borrow();
        if !sprites.is_empty() {
            let sprite_data: Vec<SpriteData> = sprites.iter().map(|s| SpriteData {
                entity_id: s.entity_id,
                texture_id: s.texture_id,
                uv: [s.u0, s.v0, s.u1, s.v1],
                color: [s.r, s.g, s.b, s.a],
                z: s.z,
            }).collect();
            engine_state.submit_sprites(sprite_data)?;
        }

        let textures = self.textures.borrow();
        if textures.is_empty() {
            let white_texture = create_test_texture(32, 32, [255, 255, 255, 255]);
            engine_state.insert_texture_with_id(1, "dummy.png", white_texture);
        } else {
            for (id, name, data) in textures.iter() {
                engine_state.insert_texture_with_id(*id, name, data.clone());
            }
        }

        renderer.render_to_virtual_canvas(&engine_state)
    }
}

#[tokio::test]
async fn test_retro_mode_320x180_rendering() -> Result<()> {
    let harness = E2ETestHarness::new();
    let script = r#"
        local entity = engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        local red = engine.rgba(255, 0, 0, 255)

        function on_start()
            engine.set_clear_color(0.2, 0.3, 0.8, 1.0) -- Blue
            engine.set_render_mode("retro")
        end

        function on_update(dt)
            engine.begin_frame()
            engine.sprite{ 
                entity = entity, 
                texture = tex, 
                pos = {160, 90}, 
                size = 64, 
                color = red,
                uv = {0,0,1,1} 
            }
            engine.end_frame()
        end
    "#;

    let fb_data = harness.execute_script(script, "retro_test").await?;
    let fb = FramebufferReader::new(&fb_data, 320, 180);

    let bg_pixel = fb.get_pixel(10, 10);
    let expected_bg = [124, 149, 231, 255]; // sRGB corrected
    assert!(pixel_matches(bg_pixel, expected_bg, 5), "Background should be blue");

    let sprite_pixel = fb.get_pixel(160, 90);
    let expected_sprite = [255, 0, 0, 255];
    assert!(pixel_matches(sprite_pixel, expected_sprite, 5), "Sprite should be red");

    Ok(())
}

#[tokio::test]
async fn test_hd_mode_1920x1080_rendering() -> Result<()> {
    let harness = E2ETestHarness::new();
    let script = r#"
        local entity = engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        local blue = engine.rgba(0, 0, 255, 255)

        function on_start()
            engine.set_clear_color(0.1, 0.5, 0.1, 1.0) -- Green
            engine.set_render_mode("hd")
        end

        function on_update(dt)
            engine.begin_frame()
            engine.sprite{ 
                entity = entity, 
                texture = tex, 
                pos = {960, 540}, 
                size = 128, 
                color = blue,
                uv = {0,0,1,1}
            }
            engine.end_frame()
        end
    "#;

    let fb_data = harness.execute_script(script, "hd_test").await?;
    let fb = FramebufferReader::new(&fb_data, 1920, 1080);

    let bg_pixel = fb.get_pixel(100, 100);
    let expected_bg = [89, 188, 89, 255]; // sRGB corrected
    assert!(pixel_matches(bg_pixel, expected_bg, 5), "Background should be green");

    let sprite_pixel = fb.get_pixel(960, 540);
    let expected_sprite = [0, 0, 255, 255];
    assert!(pixel_matches(sprite_pixel, expected_sprite, 5), "Sprite should be blue");

    Ok(())
}

#[tokio::test]
async fn test_multiple_sprites_different_positions() -> Result<()> {
    let harness = E2ETestHarness::new();
    let script = r#"
        local e1, e2, e3 = engine.create_entity(), engine.create_entity(), engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        local red = engine.rgba(255,0,0,255)
        local green = engine.rgba(0,255,0,255)
        local blue = engine.rgba(0,0,255,255)

        function on_start()
            engine.set_clear_color(0.0, 0.0, 0.0, 1.0)
            engine.set_render_mode("retro")
        end

        function on_update(dt)
            engine.begin_frame()
            engine.sprite{ entity = e1, texture = tex, pos = {80, 45}, size = 32, color = red, uv = {0,0,1,1} }
            engine.sprite{ entity = e2, texture = tex, pos = {240, 45}, size = 32, color = green, uv = {0,0,1,1} }
            engine.sprite{ entity = e3, texture = tex, pos = {160, 135}, size = 32, color = blue, uv = {0,0,1,1} }
            engine.end_frame()
        end
    "#;

    let fb_data = harness.execute_script(script, "multiple_sprites").await?;
    let fb = FramebufferReader::new(&fb_data, 320, 180);

    assert!(pixel_matches(fb.get_pixel(80, 45), [255, 0, 0, 255], 5), "Red sprite");
    assert!(pixel_matches(fb.get_pixel(240, 45), [0, 255, 0, 255], 5), "Green sprite");
    assert!(pixel_matches(fb.get_pixel(160, 135), [0, 0, 255, 255], 5), "Blue sprite");
    assert!(pixel_matches(fb.get_pixel(10, 10), [0, 0, 0, 255], 5), "Background");

    Ok(())
}

#[tokio::test]
async fn test_virtual_resolution_switching() -> Result<()> {
    let mut renderer = SpriteRenderer::new_headless(640, 480).await?;

    let mut state = EngineState::new();
    state.set_virtual_resolution(VirtualResolution::Retro320x180);
    state.set_clear_color(1.0, 0.0, 0.0, 1.0); // Red
    let retro_data = renderer.render_to_virtual_canvas(&state)?;
    assert_eq!(retro_data.len(), 320 * 180 * 4);

    state.set_virtual_resolution(VirtualResolution::Hd1920x1080);
    state.set_clear_color(0.0, 1.0, 0.0, 1.0); // Green
    let hd_data = renderer.render_to_virtual_canvas(&state)?;
    assert_eq!(hd_data.len(), 1920 * 1080 * 4);

    let retro_fb = FramebufferReader::new(&retro_data, 320, 180);
    assert!(pixel_matches(retro_fb.get_pixel(160, 90), [255, 0, 0, 255], 5));
    let hd_fb = FramebufferReader::new(&hd_data, 1920, 1080);
    assert!(pixel_matches(hd_fb.get_pixel(960, 540), [0, 255, 0, 255], 5));

    Ok(())
}

#[tokio::test]
async fn test_sprite_rotation() -> Result<()> {
    let harness = E2ETestHarness::new();
    let texture_data = {
        use image::{codecs::png::PngEncoder, ImageBuffer, Rgba};
        let img = ImageBuffer::from_fn(16, 32, |x, _y| {
            if x < 8 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 255, 255]) }
        });
        let mut png_data = Vec::new();
        PngEncoder::new(&mut png_data)
            .write_image(img.as_raw(), 16, 32, image::ColorType::Rgba8.into())
            .expect("Failed to encode PNG");
        png_data
    };
    harness.add_texture(1, "rect", texture_data);

    let script = r#"
        local entity = engine.create_entity()
        local tex = engine.load_texture("rect")
        local white = engine.rgba(255,255,255,255)

        function on_start()
            engine.set_clear_color(0.0, 0.0, 0.0, 1.0)
            engine.set_render_mode("retro")
        end

        function on_update(dt)
            engine.begin_frame()
            engine.sprite{ 
                entity = entity, 
                texture = tex, 
                pos = {160, 90}, 
                size = {32, 64}, 
                rotation = 3.14159 / 2.0, -- 90 degrees
                color = white,
                uv = {0,0,1,1}
            }
            engine.end_frame()
        end
    "#;

    let fb_data = harness.execute_script(script, "rotation_test").await?;
    let fb = FramebufferReader::new(&fb_data, 320, 180);

    let top_pixel = fb.get_pixel(160, 98);
    let bottom_pixel = fb.get_pixel(160, 82);

    assert!(pixel_matches(top_pixel, [0, 0, 255, 255], 5), "Rotated top should be blue");
    assert!(pixel_matches(bottom_pixel, [255, 0, 0, 255], 5), "Rotated bottom should be red");

    Ok(())
}

#[tokio::test]
async fn test_sprite_alpha_blending() -> Result<()> {
    let harness = E2ETestHarness::new();
    let red_texture = create_test_texture(32, 32, [255, 0, 0, 128]);
    harness.add_texture(1, "semi_red", red_texture);

    let script = r#"
        local entity = engine.create_entity()
        local tex = engine.load_texture("semi_red")
        -- Sprite color will have 50% alpha, texture has 50% alpha -> 25% total
        local sprite_color = engine.rgba(255, 255, 255, 128)

        function on_start()
            engine.set_clear_color(0.5, 0.5, 0.5, 1.0) -- Gray background
            engine.set_render_mode("retro")
        end

        function on_update(dt)
            engine.begin_frame()
            engine.sprite{ 
                entity = entity, 
                texture = tex, 
                pos = {160, 90}, 
                size = 64, 
                color = sprite_color,
                uv = {0,0,1,1}
            }
            engine.end_frame()
        end
    "#;

    let fb_data = harness.execute_script(script, "alpha_blending_test").await?;
    let fb = FramebufferReader::new(&fb_data, 320, 180);

    let blended_pixel = fb.get_pixel(160, 90);
    let expected_blend = [205, 161, 161, 255];
    assert!(
        pixel_matches(blended_pixel, expected_blend, 10),
        "Blending result is incorrect. Expected ~{:?}, got {:?}.",
        expected_blend,
        blended_pixel
    );

    Ok(())
}

#[tokio::test]
async fn test_z_ordering_wrong_submission_order() -> Result<()> {
    let harness = E2ETestHarness::new();
    let script = r#"
        local front = engine.create_entity()
        local back = engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        local blue = engine.rgba(0,0,255,255)
        local red = engine.rgba(255,0,0,255)

        function on_start()
            engine.set_clear_color(0.0, 0.0, 0.0, 1.0)
            engine.set_render_mode("retro")
        end

        function on_update(dt)
            engine.begin_frame()
            -- Draw back sprite first, then front sprite
            engine.sprite{ entity = back, texture = tex, pos = {160,90}, size=64, color=red, z=0.0, uv={0,0,1,1} }
            engine.sprite{ entity = front, texture = tex, pos = {160,90}, size=64, color=blue, z=10.0, uv={0,0,1,1} }
            engine.end_frame()
        end
    "#;

    let fb_data = harness.execute_script(script, "z_order_test").await?;
    let fb = FramebufferReader::new(&fb_data, 320, 180);

    let center_pixel = fb.get_pixel(160, 90);
    assert!(pixel_matches(center_pixel, [0, 0, 255, 255], 5), "Z-ordering failed, expected blue on top");

    Ok(())
}

#[tokio::test]
async fn test_z_ordering_partial_overlaps() -> Result<()> {
    let harness = E2ETestHarness::new();
    let script = r#"
        local high_z = engine.create_entity()
        local mid_z = engine.create_entity()
        local low_z = engine.create_entity()
        local tex = engine.load_texture("dummy.png")
        local blue = engine.rgba(0,0,255,255)
        local green = engine.rgba(0,255,0,255)
        local red = engine.rgba(255,0,0,255)

        function on_start()
            engine.set_clear_color(0.0, 0.0, 0.0, 1.0)
            engine.set_render_mode("retro")
        end

        function on_update(dt)
            engine.begin_frame()
            engine.sprite{ entity=low_z, texture=tex, pos={140,90}, size=60, color=red, z=1.0, uv={0,0,1,1} }
            engine.sprite{ entity=mid_z, texture=tex, pos={160,90}, size=60, color=green, z=3.0, uv={0,0,1,1} }
            engine.sprite{ entity=high_z, texture=tex, pos={180,90}, size=60, color=blue, z=5.0, uv={0,0,1,1} }
            engine.end_frame()
        end
    "#;

    let fb_data = harness.execute_script(script, "partial_overlap_test").await?;
    let fb = FramebufferReader::new(&fb_data, 320, 180);

    assert!(pixel_matches(fb.get_pixel(120, 90), [255, 0, 0, 255], 5), "Red only");
    assert!(pixel_matches(fb.get_pixel(140, 90), [0, 255, 0, 255], 5), "Green over Red");
    assert!(pixel_matches(fb.get_pixel(160, 90), [0, 0, 255, 255], 5), "Blue over Green/Red");
    assert!(pixel_matches(fb.get_pixel(180, 90), [0, 0, 255, 255], 5), "Blue over Green");
    assert!(pixel_matches(fb.get_pixel(200, 90), [0, 0, 255, 255], 5), "Blue only");

    Ok(())
}
