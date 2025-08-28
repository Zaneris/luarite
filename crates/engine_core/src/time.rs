use std::time::Instant;

const FIXED_TIMESTEP: f64 = 1.0 / 60.0; // 60 FPS fixed timestep
const MAX_FRAME_TIME: f64 = 0.25; // Don't spiral of death beyond 4 FPS

pub struct FixedTimeStep {
    fixed_dt: f64,
    accumulator: f64,
    current_time: Instant,
    fixed_time: f64, // Deterministic time for scripts
}

impl FixedTimeStep {
    pub fn new() -> Self {
        Self {
            fixed_dt: FIXED_TIMESTEP,
            accumulator: 0.0,
            current_time: Instant::now(),
            fixed_time: 0.0,
        }
    }

    pub fn update<F>(&mut self, mut fixed_update: F) 
    where 
        F: FnMut(f64),
    {
        let new_time = Instant::now();
        let frame_time = (new_time - self.current_time).as_secs_f64();
        self.current_time = new_time;

        // Clamp frame time to prevent spiral of death
        let clamped_frame_time = frame_time.min(MAX_FRAME_TIME);
        self.accumulator += clamped_frame_time;

        // Run fixed timestep updates
        while self.accumulator >= self.fixed_dt {
            fixed_update(self.fixed_dt);
            self.fixed_time += self.fixed_dt;
            self.accumulator -= self.fixed_dt;
        }
    }

    pub fn fixed_dt(&self) -> f64 {
        self.fixed_dt
    }

    pub fn fixed_time(&self) -> f64 {
        self.fixed_time
    }

    pub fn alpha(&self) -> f64 {
        self.accumulator / self.fixed_dt
    }
}