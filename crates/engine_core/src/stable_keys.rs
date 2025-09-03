// Stable key codes for input consistency across engine versions.
//
// These values are guaranteed to remain stable across winit updates,
// ensuring that replays and persistence work correctly.

// Letters (0x0001 - 0x001A)
pub const KEY_A: u32 = 0x0001;
pub const KEY_B: u32 = 0x0002;
pub const KEY_C: u32 = 0x0003;
pub const KEY_D: u32 = 0x0004;
pub const KEY_E: u32 = 0x0005;
pub const KEY_F: u32 = 0x0006;
pub const KEY_G: u32 = 0x0007;
pub const KEY_H: u32 = 0x0008;
pub const KEY_I: u32 = 0x0009;
pub const KEY_J: u32 = 0x000A;
pub const KEY_K: u32 = 0x000B;
pub const KEY_L: u32 = 0x000C;
pub const KEY_M: u32 = 0x000D;
pub const KEY_N: u32 = 0x000E;
pub const KEY_O: u32 = 0x000F;
pub const KEY_P: u32 = 0x0010;
pub const KEY_Q: u32 = 0x0011;
pub const KEY_R: u32 = 0x0012;
pub const KEY_S: u32 = 0x0013;
pub const KEY_T: u32 = 0x0014;
pub const KEY_U: u32 = 0x0015;
pub const KEY_V: u32 = 0x0016;
pub const KEY_W: u32 = 0x0017;
pub const KEY_X: u32 = 0x0018;
pub const KEY_Y: u32 = 0x0019;
pub const KEY_Z: u32 = 0x001A;

// Digits (0x0030 - 0x0039, matching ASCII for easy debugging)
pub const DIGIT_0: u32 = 0x0030;
pub const DIGIT_1: u32 = 0x0031;
pub const DIGIT_2: u32 = 0x0032;
pub const DIGIT_3: u32 = 0x0033;
pub const DIGIT_4: u32 = 0x0034;
pub const DIGIT_5: u32 = 0x0035;
pub const DIGIT_6: u32 = 0x0036;
pub const DIGIT_7: u32 = 0x0037;
pub const DIGIT_8: u32 = 0x0038;
pub const DIGIT_9: u32 = 0x0039;

// Symbols/Punctuation (0x0100 - 0x011F)
pub const BACKQUOTE: u32 = 0x0100;
pub const BACKSLASH: u32 = 0x0101;
pub const BRACKET_LEFT: u32 = 0x0102;
pub const BRACKET_RIGHT: u32 = 0x0103;
pub const COMMA: u32 = 0x0104;
pub const EQUAL: u32 = 0x0105;
pub const MINUS: u32 = 0x0106;
pub const PERIOD: u32 = 0x0107;
pub const QUOTE: u32 = 0x0108;
pub const SEMICOLON: u32 = 0x0109;
pub const SLASH: u32 = 0x010A;

// Modifiers (0x0200 - 0x020F)
pub const ALT_LEFT: u32 = 0x0200;
pub const ALT_RIGHT: u32 = 0x0201;
pub const CONTROL_LEFT: u32 = 0x0202;
pub const CONTROL_RIGHT: u32 = 0x0203;
pub const SHIFT_LEFT: u32 = 0x0204;
pub const SHIFT_RIGHT: u32 = 0x0205;
pub const SUPER_LEFT: u32 = 0x0206;
pub const SUPER_RIGHT: u32 = 0x0207;

// Special keys (0x0300 - 0x030F)
pub const BACKSPACE: u32 = 0x0300;
pub const CAPS_LOCK: u32 = 0x0301;
pub const CONTEXT_MENU: u32 = 0x0302;
pub const ENTER: u32 = 0x0303;
pub const SPACE: u32 = 0x0304;
pub const TAB: u32 = 0x0305;

// Arrow keys (0x0400 - 0x0403)
pub const ARROW_DOWN: u32 = 0x0400;
pub const ARROW_LEFT: u32 = 0x0401;
pub const ARROW_RIGHT: u32 = 0x0402;
pub const ARROW_UP: u32 = 0x0403;

// Navigation (0x0500 - 0x050F)
pub const END: u32 = 0x0500;
pub const HOME: u32 = 0x0501;
pub const PAGE_DOWN: u32 = 0x0502;
pub const PAGE_UP: u32 = 0x0503;

// Function keys (0x0600 - 0x060B)
pub const F1: u32 = 0x0600;
pub const F2: u32 = 0x0601;
pub const F3: u32 = 0x0602;
pub const F4: u32 = 0x0603;
pub const F5: u32 = 0x0604;
pub const F6: u32 = 0x0605;
pub const F7: u32 = 0x0606;
pub const F8: u32 = 0x0607;
pub const F9: u32 = 0x0608;
pub const F10: u32 = 0x0609;
pub const F11: u32 = 0x060A;
pub const F12: u32 = 0x060B;

// Numpad (0x0700 - 0x071F)
pub const NUMPAD_0: u32 = 0x0700;
pub const NUMPAD_1: u32 = 0x0701;
pub const NUMPAD_2: u32 = 0x0702;
pub const NUMPAD_3: u32 = 0x0703;
pub const NUMPAD_4: u32 = 0x0704;
pub const NUMPAD_5: u32 = 0x0705;
pub const NUMPAD_6: u32 = 0x0706;
pub const NUMPAD_7: u32 = 0x0707;
pub const NUMPAD_8: u32 = 0x0708;
pub const NUMPAD_9: u32 = 0x0709;
pub const NUMPAD_ADD: u32 = 0x070A;
pub const NUMPAD_DECIMAL: u32 = 0x070B;
pub const NUMPAD_DIVIDE: u32 = 0x070C;
pub const NUMPAD_MULTIPLY: u32 = 0x070D;
pub const NUMPAD_SUBTRACT: u32 = 0x070E;
pub const NUMPAD_ENTER: u32 = 0x070F;

// International (0x0800 - 0x080F)
pub const INTL_BACKSLASH: u32 = 0x0800;
pub const INTL_RO: u32 = 0x0801;
pub const INTL_YEN: u32 = 0x0802;

/// Convert winit KeyCode to stable key code.
/// Returns None for unsupported keys.
pub fn winit_to_stable(key: winit::keyboard::KeyCode) -> Option<u32> {
    use winit::keyboard::KeyCode;

    match key {
        // Letters
        KeyCode::KeyA => Some(KEY_A),
        KeyCode::KeyB => Some(KEY_B),
        KeyCode::KeyC => Some(KEY_C),
        KeyCode::KeyD => Some(KEY_D),
        KeyCode::KeyE => Some(KEY_E),
        KeyCode::KeyF => Some(KEY_F),
        KeyCode::KeyG => Some(KEY_G),
        KeyCode::KeyH => Some(KEY_H),
        KeyCode::KeyI => Some(KEY_I),
        KeyCode::KeyJ => Some(KEY_J),
        KeyCode::KeyK => Some(KEY_K),
        KeyCode::KeyL => Some(KEY_L),
        KeyCode::KeyM => Some(KEY_M),
        KeyCode::KeyN => Some(KEY_N),
        KeyCode::KeyO => Some(KEY_O),
        KeyCode::KeyP => Some(KEY_P),
        KeyCode::KeyQ => Some(KEY_Q),
        KeyCode::KeyR => Some(KEY_R),
        KeyCode::KeyS => Some(KEY_S),
        KeyCode::KeyT => Some(KEY_T),
        KeyCode::KeyU => Some(KEY_U),
        KeyCode::KeyV => Some(KEY_V),
        KeyCode::KeyW => Some(KEY_W),
        KeyCode::KeyX => Some(KEY_X),
        KeyCode::KeyY => Some(KEY_Y),
        KeyCode::KeyZ => Some(KEY_Z),

        // Digits
        KeyCode::Digit0 => Some(DIGIT_0),
        KeyCode::Digit1 => Some(DIGIT_1),
        KeyCode::Digit2 => Some(DIGIT_2),
        KeyCode::Digit3 => Some(DIGIT_3),
        KeyCode::Digit4 => Some(DIGIT_4),
        KeyCode::Digit5 => Some(DIGIT_5),
        KeyCode::Digit6 => Some(DIGIT_6),
        KeyCode::Digit7 => Some(DIGIT_7),
        KeyCode::Digit8 => Some(DIGIT_8),
        KeyCode::Digit9 => Some(DIGIT_9),

        // Symbols
        KeyCode::Backquote => Some(BACKQUOTE),
        KeyCode::Backslash => Some(BACKSLASH),
        KeyCode::BracketLeft => Some(BRACKET_LEFT),
        KeyCode::BracketRight => Some(BRACKET_RIGHT),
        KeyCode::Comma => Some(COMMA),
        KeyCode::Equal => Some(EQUAL),
        KeyCode::Minus => Some(MINUS),
        KeyCode::Period => Some(PERIOD),
        KeyCode::Quote => Some(QUOTE),
        KeyCode::Semicolon => Some(SEMICOLON),
        KeyCode::Slash => Some(SLASH),

        // Modifiers
        KeyCode::AltLeft => Some(ALT_LEFT),
        KeyCode::AltRight => Some(ALT_RIGHT),
        KeyCode::ControlLeft => Some(CONTROL_LEFT),
        KeyCode::ControlRight => Some(CONTROL_RIGHT),
        KeyCode::ShiftLeft => Some(SHIFT_LEFT),
        KeyCode::ShiftRight => Some(SHIFT_RIGHT),
        KeyCode::SuperLeft => Some(SUPER_LEFT),
        KeyCode::SuperRight => Some(SUPER_RIGHT),

        // Special
        KeyCode::Backspace => Some(BACKSPACE),
        KeyCode::CapsLock => Some(CAPS_LOCK),
        KeyCode::ContextMenu => Some(CONTEXT_MENU),
        KeyCode::Enter => Some(ENTER),
        KeyCode::Space => Some(SPACE),
        KeyCode::Tab => Some(TAB),

        // Arrows
        KeyCode::ArrowDown => Some(ARROW_DOWN),
        KeyCode::ArrowLeft => Some(ARROW_LEFT),
        KeyCode::ArrowRight => Some(ARROW_RIGHT),
        KeyCode::ArrowUp => Some(ARROW_UP),

        // Navigation
        KeyCode::End => Some(END),
        KeyCode::Home => Some(HOME),
        KeyCode::PageDown => Some(PAGE_DOWN),
        KeyCode::PageUp => Some(PAGE_UP),

        // Function keys
        KeyCode::F1 => Some(F1),
        KeyCode::F2 => Some(F2),
        KeyCode::F3 => Some(F3),
        KeyCode::F4 => Some(F4),
        KeyCode::F5 => Some(F5),
        KeyCode::F6 => Some(F6),
        KeyCode::F7 => Some(F7),
        KeyCode::F8 => Some(F8),
        KeyCode::F9 => Some(F9),
        KeyCode::F10 => Some(F10),
        KeyCode::F11 => Some(F11),
        KeyCode::F12 => Some(F12),

        // Numpad
        KeyCode::Numpad0 => Some(NUMPAD_0),
        KeyCode::Numpad1 => Some(NUMPAD_1),
        KeyCode::Numpad2 => Some(NUMPAD_2),
        KeyCode::Numpad3 => Some(NUMPAD_3),
        KeyCode::Numpad4 => Some(NUMPAD_4),
        KeyCode::Numpad5 => Some(NUMPAD_5),
        KeyCode::Numpad6 => Some(NUMPAD_6),
        KeyCode::Numpad7 => Some(NUMPAD_7),
        KeyCode::Numpad8 => Some(NUMPAD_8),
        KeyCode::Numpad9 => Some(NUMPAD_9),
        KeyCode::NumpadAdd => Some(NUMPAD_ADD),
        KeyCode::NumpadDecimal => Some(NUMPAD_DECIMAL),
        KeyCode::NumpadDivide => Some(NUMPAD_DIVIDE),
        KeyCode::NumpadMultiply => Some(NUMPAD_MULTIPLY),
        KeyCode::NumpadSubtract => Some(NUMPAD_SUBTRACT),
        KeyCode::NumpadEnter => Some(NUMPAD_ENTER),

        // International
        KeyCode::IntlBackslash => Some(INTL_BACKSLASH),
        KeyCode::IntlRo => Some(INTL_RO),
        KeyCode::IntlYen => Some(INTL_YEN),

        // Unsupported keys
        _ => None,
    }
}

/// Convert stable key code back to winit KeyCode.
/// Returns None for invalid or unsupported stable codes.
pub fn stable_to_winit(stable_key: u32) -> Option<winit::keyboard::KeyCode> {
    use winit::keyboard::KeyCode;

    match stable_key {
        // Letters
        KEY_A => Some(KeyCode::KeyA),
        KEY_B => Some(KeyCode::KeyB),
        KEY_C => Some(KeyCode::KeyC),
        KEY_D => Some(KeyCode::KeyD),
        KEY_E => Some(KeyCode::KeyE),
        KEY_F => Some(KeyCode::KeyF),
        KEY_G => Some(KeyCode::KeyG),
        KEY_H => Some(KeyCode::KeyH),
        KEY_I => Some(KeyCode::KeyI),
        KEY_J => Some(KeyCode::KeyJ),
        KEY_K => Some(KeyCode::KeyK),
        KEY_L => Some(KeyCode::KeyL),
        KEY_M => Some(KeyCode::KeyM),
        KEY_N => Some(KeyCode::KeyN),
        KEY_O => Some(KeyCode::KeyO),
        KEY_P => Some(KeyCode::KeyP),
        KEY_Q => Some(KeyCode::KeyQ),
        KEY_R => Some(KeyCode::KeyR),
        KEY_S => Some(KeyCode::KeyS),
        KEY_T => Some(KeyCode::KeyT),
        KEY_U => Some(KeyCode::KeyU),
        KEY_V => Some(KeyCode::KeyV),
        KEY_W => Some(KeyCode::KeyW),
        KEY_X => Some(KeyCode::KeyX),
        KEY_Y => Some(KeyCode::KeyY),
        KEY_Z => Some(KeyCode::KeyZ),

        // Digits
        DIGIT_0 => Some(KeyCode::Digit0),
        DIGIT_1 => Some(KeyCode::Digit1),
        DIGIT_2 => Some(KeyCode::Digit2),
        DIGIT_3 => Some(KeyCode::Digit3),
        DIGIT_4 => Some(KeyCode::Digit4),
        DIGIT_5 => Some(KeyCode::Digit5),
        DIGIT_6 => Some(KeyCode::Digit6),
        DIGIT_7 => Some(KeyCode::Digit7),
        DIGIT_8 => Some(KeyCode::Digit8),
        DIGIT_9 => Some(KeyCode::Digit9),

        // Symbols
        BACKQUOTE => Some(KeyCode::Backquote),
        BACKSLASH => Some(KeyCode::Backslash),
        BRACKET_LEFT => Some(KeyCode::BracketLeft),
        BRACKET_RIGHT => Some(KeyCode::BracketRight),
        COMMA => Some(KeyCode::Comma),
        EQUAL => Some(KeyCode::Equal),
        MINUS => Some(KeyCode::Minus),
        PERIOD => Some(KeyCode::Period),
        QUOTE => Some(KeyCode::Quote),
        SEMICOLON => Some(KeyCode::Semicolon),
        SLASH => Some(KeyCode::Slash),

        // Modifiers
        ALT_LEFT => Some(KeyCode::AltLeft),
        ALT_RIGHT => Some(KeyCode::AltRight),
        CONTROL_LEFT => Some(KeyCode::ControlLeft),
        CONTROL_RIGHT => Some(KeyCode::ControlRight),
        SHIFT_LEFT => Some(KeyCode::ShiftLeft),
        SHIFT_RIGHT => Some(KeyCode::ShiftRight),
        SUPER_LEFT => Some(KeyCode::SuperLeft),
        SUPER_RIGHT => Some(KeyCode::SuperRight),

        // Special
        BACKSPACE => Some(KeyCode::Backspace),
        CAPS_LOCK => Some(KeyCode::CapsLock),
        CONTEXT_MENU => Some(KeyCode::ContextMenu),
        ENTER => Some(KeyCode::Enter),
        SPACE => Some(KeyCode::Space),
        TAB => Some(KeyCode::Tab),

        // Arrows
        ARROW_DOWN => Some(KeyCode::ArrowDown),
        ARROW_LEFT => Some(KeyCode::ArrowLeft),
        ARROW_RIGHT => Some(KeyCode::ArrowRight),
        ARROW_UP => Some(KeyCode::ArrowUp),

        // Navigation
        END => Some(KeyCode::End),
        HOME => Some(KeyCode::Home),
        PAGE_DOWN => Some(KeyCode::PageDown),
        PAGE_UP => Some(KeyCode::PageUp),

        // Function keys
        F1 => Some(KeyCode::F1),
        F2 => Some(KeyCode::F2),
        F3 => Some(KeyCode::F3),
        F4 => Some(KeyCode::F4),
        F5 => Some(KeyCode::F5),
        F6 => Some(KeyCode::F6),
        F7 => Some(KeyCode::F7),
        F8 => Some(KeyCode::F8),
        F9 => Some(KeyCode::F9),
        F10 => Some(KeyCode::F10),
        F11 => Some(KeyCode::F11),
        F12 => Some(KeyCode::F12),

        // Numpad
        NUMPAD_0 => Some(KeyCode::Numpad0),
        NUMPAD_1 => Some(KeyCode::Numpad1),
        NUMPAD_2 => Some(KeyCode::Numpad2),
        NUMPAD_3 => Some(KeyCode::Numpad3),
        NUMPAD_4 => Some(KeyCode::Numpad4),
        NUMPAD_5 => Some(KeyCode::Numpad5),
        NUMPAD_6 => Some(KeyCode::Numpad6),
        NUMPAD_7 => Some(KeyCode::Numpad7),
        NUMPAD_8 => Some(KeyCode::Numpad8),
        NUMPAD_9 => Some(KeyCode::Numpad9),
        NUMPAD_ADD => Some(KeyCode::NumpadAdd),
        NUMPAD_DECIMAL => Some(KeyCode::NumpadDecimal),
        NUMPAD_DIVIDE => Some(KeyCode::NumpadDivide),
        NUMPAD_MULTIPLY => Some(KeyCode::NumpadMultiply),
        NUMPAD_SUBTRACT => Some(KeyCode::NumpadSubtract),
        NUMPAD_ENTER => Some(KeyCode::NumpadEnter),

        // International
        INTL_BACKSLASH => Some(KeyCode::IntlBackslash),
        INTL_RO => Some(KeyCode::IntlRo),
        INTL_YEN => Some(KeyCode::IntlYen),

        // Invalid
        _ => None,
    }
}
