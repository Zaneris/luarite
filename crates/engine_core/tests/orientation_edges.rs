use engine_core::offscreen::OffscreenRenderer;
use engine_core::state::{EngineState, SpriteData};

// Draw quads at screen edges and verify Y-up orientation and no culling issues.
#[test]
fn quads_visible_at_edges_y_up() {
    let w = 128u32;
    let h = 128u32;
    let mut state = EngineState::new();
    state.set_window_size(w, h);

    // Four entities: corners (32px half-size = 64x64 quads)
    // Positions in pixels; Y-up means bottom = 0, top = h
    let mut t: Vec<f64> = Vec::new();
    // bottom-left at (32, 32)
    t.extend_from_slice(&[1.0, 32.0, 32.0, 0.0, 1.0, 1.0]);
    // top-left at (32, h-32)
    t.extend_from_slice(&[2.0, 32.0, (h as f64) - 32.0, 0.0, 1.0, 1.0]);
    // top-right at (w-32, h-32)
    t.extend_from_slice(&[3.0, (w as f64) - 32.0, (h as f64) - 32.0, 0.0, 1.0, 1.0]);
    // bottom-right at (w-32, 32)
    t.extend_from_slice(&[4.0, (w as f64) - 32.0, 32.0, 0.0, 1.0, 1.0]);
    state.set_transforms_from_slice(&t).unwrap();
    let mut sprites = vec![
        SpriteData { entity_id: 1, texture_id: 0, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] },
        SpriteData { entity_id: 2, texture_id: 0, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] },
        SpriteData { entity_id: 3, texture_id: 0, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] },
        SpriteData { entity_id: 4, texture_id: 0, uv: [0.0, 0.0, 1.0, 1.0], color: [1.0, 0.0, 1.0, 1.0] },
    ];
    state.append_sprites(&mut sprites).unwrap();

    let rdr = pollster::block_on(OffscreenRenderer::new(w, h)).unwrap();
    let rgba = rdr.render_state_to_rgba(&state).unwrap();
    let mut sample = |x: u32, y: u32| {
        let idx = ((y * w + x) * 4) as usize;
        (rgba[idx] as i32, rgba[idx + 1] as i32, rgba[idx + 2] as i32)
    };
    // Sample inside each quad
    let (r1, g1, b1) = sample(32, 32); // bottom-left center
    let (r2, g2, b2) = sample(32, h - 32); // top-left center
    let (r3, g3, b3) = sample(w - 32, h - 32); // top-right center
    let (r4, g4, b4) = sample(w - 32, 32); // bottom-right center
    for (ri, gi, bi) in [(r1,g1,b1), (r2,g2,b2), (r3,g3,b3), (r4,g4,b4)] {
        assert!(ri > 180 && gi < 60 && bi > 180, "unexpected color {},{},{}", ri, gi, bi);
    }
}

