use engine_core::offscreen::OffscreenRenderer;
use engine_core::state::{EngineState, SpriteData};

#[test]
fn renders_magenta_quad() {
    let mut state = EngineState::new();
    state.set_window_size(256, 256);
    // One entity at center with 64x64 size
    let cx = 128.0f32; let cy = 128.0f32;
    let transforms: Vec<f64> = vec![1.0, cx as f64, cy as f64, 0.0, 1.0, 1.0];
    state.set_transforms_from_slice(&transforms).unwrap();
    let mut sprites = vec![SpriteData { entity_id: 1, texture_id: 0, uv: [0.0,0.0,1.0,1.0], color: [1.0, 0.0, 1.0, 1.0] }];
    state.append_sprites(&mut sprites).unwrap();

    let rdr = pollster::block_on(OffscreenRenderer::new(256, 256)).unwrap();
    let rgba = rdr.render_state_to_rgba(&state).unwrap();
    // Sample the center pixel
    let x = 128u32; let y = 128u32; let idx = ((y * 256 + x) * 4) as usize;
    let r = rgba[idx] as i32; let g = rgba[idx+1] as i32; let b = rgba[idx+2] as i32;
    // Expect magenta-ish (white*magenta tint)
    assert!(r > 200 && g < 30 && b > 200, "unexpected color at center: {},{},{}", r,g,b);
}
