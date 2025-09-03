use crate::state::VirtualResolution;

/// Calculates the basic scale factor to fit virtual canvas into window
pub fn calculate_base_scale(
    window_width: f32,
    window_height: f32,
    virtual_width: f32,
    virtual_height: f32,
) -> f32 {
    (window_width / virtual_width).min(window_height / virtual_height)
}

/// Scaling result containing the final scale and whether to use linear filtering
#[derive(Debug, PartialEq)]
pub struct ScalingResult {
    pub scale: f32,
    pub use_linear_filtering: bool,
}

/// Calculates the final scaling factor and filtering mode based on virtual resolution mode
pub fn calculate_final_scaling(base_scale: f32, virtual_mode: VirtualResolution) -> ScalingResult {
    match virtual_mode {
        VirtualResolution::Retro320x180 => {
            // Retro mode: floor to integer scale (min 1.0), always use nearest neighbor
            ScalingResult {
                scale: base_scale.floor().max(1.0),
                use_linear_filtering: false,
            }
        }
        VirtualResolution::Hd1920x1080 => {
            // HD mode: use integer scaling if within 5% tolerance, otherwise linear
            let ideal_scale = (base_scale * 100.0).round() / 100.0;
            let nearest_int_scale = base_scale.round();

            if (ideal_scale - nearest_int_scale).abs() < 0.05 {
                ScalingResult {
                    scale: nearest_int_scale,
                    use_linear_filtering: false,
                }
            } else {
                ScalingResult {
                    scale: base_scale,
                    use_linear_filtering: true,
                }
            }
        }
    }
}

/// Letterboxing result containing the position and size of the scaled virtual canvas
#[derive(Debug, PartialEq)]
pub struct LetterboxResult {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Calculates letterboxing position and size for the scaled virtual canvas within the window
pub fn calculate_letterbox_rect(
    window_width: f32,
    window_height: f32,
    virtual_width: f32,
    virtual_height: f32,
    final_scale: f32,
) -> LetterboxResult {
    let scaled_width = virtual_width * final_scale;
    let scaled_height = virtual_height * final_scale;

    // Center the scaled canvas in the window
    let x = (window_width - scaled_width) * 0.5;
    let y = (window_height - scaled_height) * 0.5;

    LetterboxResult {
        x,
        y,
        width: scaled_width,
        height: scaled_height,
    }
}

/// Converts pixel coordinates to normalized device coordinates (-1.0 to 1.0)
pub fn pixel_to_ndc(pixel_coord: f32, dimension_size: f32) -> f32 {
    (pixel_coord / dimension_size) * 2.0 - 1.0
}

/// Complete present pass calculation combining all steps
pub fn calculate_present_pass_transform(
    window_width: f32,
    window_height: f32,
    virtual_width: f32,
    virtual_height: f32,
    virtual_mode: VirtualResolution,
) -> (LetterboxResult, bool) {
    let base_scale =
        calculate_base_scale(window_width, window_height, virtual_width, virtual_height);
    let scaling = calculate_final_scaling(base_scale, virtual_mode);
    let letterbox = calculate_letterbox_rect(
        window_width,
        window_height,
        virtual_width,
        virtual_height,
        scaling.scale,
    );

    (letterbox, scaling.use_linear_filtering)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_scale_calculation() {
        // Window wider than virtual canvas (height is limiting factor)
        // width_scale = 800/320 = 2.5, height_scale = 600/180 = 3.33, min = 2.5
        assert_eq!(calculate_base_scale(800.0, 600.0, 320.0, 180.0), 2.5);

        // Window taller than virtual canvas (width is limiting factor)
        // width_scale = 400/320 = 1.25, height_scale = 800/180 = 4.44, min = 1.25
        assert_eq!(calculate_base_scale(400.0, 800.0, 320.0, 180.0), 1.25);

        // Perfect aspect ratio match
        assert_eq!(calculate_base_scale(640.0, 360.0, 320.0, 180.0), 2.0);
    }

    #[test]
    fn test_retro_scaling() {
        // Should floor and use nearest neighbor
        let result = calculate_final_scaling(2.7, VirtualResolution::Retro320x180);
        assert_eq!(result.scale, 2.0);
        assert!(!result.use_linear_filtering);

        // Should enforce minimum scale of 1.0
        let result = calculate_final_scaling(0.5, VirtualResolution::Retro320x180);
        assert_eq!(result.scale, 1.0);
        assert!(!result.use_linear_filtering);
    }

    #[test]
    fn test_hd_scaling_integer_tolerance() {
        // Exactly 2.0 scale should use nearest neighbor
        let result = calculate_final_scaling(2.0, VirtualResolution::Hd1920x1080);
        assert_eq!(result.scale, 2.0);
        assert!(!result.use_linear_filtering);

        // Within 5% tolerance (1.97) should round to integer
        let result = calculate_final_scaling(1.97, VirtualResolution::Hd1920x1080);
        assert_eq!(result.scale, 2.0);
        assert!(!result.use_linear_filtering);

        // Outside tolerance should use linear
        let result = calculate_final_scaling(1.5, VirtualResolution::Hd1920x1080);
        assert_eq!(result.scale, 1.5);
        assert!(result.use_linear_filtering);
    }

    #[test]
    fn test_letterbox_calculation() {
        let result = calculate_letterbox_rect(800.0, 600.0, 320.0, 180.0, 2.0);

        // Scaled canvas: 640x360
        // Centered in 800x600 window: x=(800-640)/2=80, y=(600-360)/2=120
        assert_eq!(result.x, 80.0);
        assert_eq!(result.y, 120.0);
        assert_eq!(result.width, 640.0);
        assert_eq!(result.height, 360.0);
    }

    #[test]
    fn test_pixel_to_ndc() {
        // Left edge of 800px window
        assert_eq!(pixel_to_ndc(0.0, 800.0), -1.0);

        // Right edge of 800px window
        assert_eq!(pixel_to_ndc(800.0, 800.0), 1.0);

        // Center of 800px window
        assert_eq!(pixel_to_ndc(400.0, 800.0), 0.0);
    }
}
