use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct InputState {
    pub keys: HashSet<String>,
    pub mouse_buttons: HashSet<String>,
    pub mouse_x: f64,
    pub mouse_y: f64,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys: HashSet::new(),
            mouse_buttons: HashSet::new(),
            mouse_x: 0.0,
            mouse_y: 0.0,
        }
    }

    pub fn clear(&mut self) {}

    pub fn set_mouse_pos(&mut self, x: f64, y: f64) {
        self.mouse_x = x;
        self.mouse_y = y;
    }

    pub fn set_key(&mut self, name: String, down: bool) {
        if down {
            self.keys.insert(name);
        } else {
            self.keys.remove(&name);
        }
    }

    pub fn set_mouse_button(&mut self, name: String, down: bool) {
        if down {
            self.mouse_buttons.insert(name);
        } else {
            self.mouse_buttons.remove(&name);
        }
    }
}
