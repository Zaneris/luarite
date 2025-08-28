use anyhow::Result;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
    dpi::PhysicalSize,
};
use std::sync::Arc;
use crate::time::FixedTimeStep;
use crate::metrics::MetricsCollector;

pub struct EngineWindow {
    window: Option<Arc<Window>>,
    timestep: FixedTimeStep,
    metrics: MetricsCollector,
    frame_count: u64,
}

impl EngineWindow {
    pub fn new() -> Self {
        Self { 
            window: None,
            timestep: FixedTimeStep::new(),
            metrics: MetricsCollector::new(),
            frame_count: 0,
        }
    }

    pub fn run(mut self) -> Result<()> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self)?;
        Ok(())
    }

    pub fn window(&self) -> Option<&Arc<Window>> {
        self.window.as_ref()
    }
}

impl ApplicationHandler for EngineWindow {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = WindowAttributes::default()
                .with_title("Luarite Engine")
                .with_inner_size(PhysicalSize::new(1024, 768));

            match event_loop.create_window(window_attributes) {
                Ok(window) => {
                    tracing::info!("Window created successfully");
                    self.window = Some(Arc::new(window));
                }
                Err(e) => {
                    tracing::error!("Failed to create window: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                tracing::info!("Window close requested");
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                tracing::debug!("Window resized: {:?}", physical_size);
            }
            WindowEvent::RedrawRequested => {
                // TODO: Render frame
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Begin frame metrics collection
        self.metrics.begin_frame();
        self.frame_count += 1;

        // Run fixed timestep updates
        let mut updates_run = 0;
        self.timestep.update(|dt| {
            updates_run += 1;
            // This runs at exactly 60 FPS for deterministic behavior
            if updates_run <= 5 { // Limit debug spam
                tracing::debug!("Fixed update: dt={:.6}s", dt);
            }
            // TODO: Update game logic here (will call Lua scripts later)
        });

        // End frame metrics collection
        self.metrics.end_frame();

        // Log metrics every 5 seconds (300 frames at 60 FPS)
        if self.frame_count % 300 == 0 {
            let stats = self.metrics.get_performance_stats();
            let violations = self.metrics.validate_performance_budgets();
            
            tracing::info!(
                "Performance stats ({}s): CPU mean={:.2}ms, p99={:.2}ms, FFI calls={:.1}",
                self.frame_count / 60,
                stats.get("cpu_frame_mean_ms").unwrap_or(&0.0),
                stats.get("cpu_frame_p99_ms").unwrap_or(&0.0),
                stats.get("ffi_calls_max").unwrap_or(&0.0),
            );

            for violation in violations {
                tracing::warn!("Performance budget violation: {}", violation);
            }
        }

        // Request redraw
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}