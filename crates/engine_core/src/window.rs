use crate::metrics::MetricsCollector;
use crate::renderer::SpriteRenderer;
use crate::state::EngineState;
use crate::time::FixedTimeStep;
use anyhow::Result;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};

type OnStartCb = Box<dyn FnMut(&mut EngineState)>;
type OnUpdateCb = Box<dyn FnMut(f64, &mut EngineState)>;

pub struct EngineWindow {
    window: Option<Arc<Window>>,
    renderer: Option<SpriteRenderer>,
    engine_state: EngineState,
    timestep: FixedTimeStep,
    metrics: MetricsCollector,
    frame_count: u64,
    // Optional script hooks provided by host
    script_on_start_called: bool,
    script_on_start: Option<OnStartCb>,
    script_on_update: Option<OnUpdateCb>,
}

impl EngineWindow {
    pub fn new() -> Self {
        Self {
            window: None,
            renderer: None,
            engine_state: EngineState::new(),
            timestep: FixedTimeStep::new(),
            metrics: MetricsCollector::new(),
            frame_count: 0,
            script_on_start_called: false,
            script_on_start: None,
            script_on_update: None,
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

    pub fn engine_state(&mut self) -> &mut EngineState {
        &mut self.engine_state
    }

    // Host integration: set optional script callbacks
    pub fn set_script_on_start<F>(&mut self, f: F)
    where
        F: FnMut(&mut EngineState) + 'static,
    {
        self.script_on_start = Some(Box::new(f));
    }

    pub fn set_script_on_update<F>(&mut self, f: F)
    where
        F: FnMut(f64, &mut EngineState) + 'static,
    {
        self.script_on_update = Some(Box::new(f));
    }
}

impl Default for EngineWindow {
    fn default() -> Self {
        Self::new()
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
                    let window_arc = Arc::new(window);
                    tracing::info!("Window created successfully");

                    // Initialize renderer (this is async, we'll handle it in about_to_wait)
                    self.window = Some(window_arc);
                }
                Err(e) => {
                    tracing::error!("Failed to create window: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                tracing::info!("Window close requested");
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                tracing::debug!("Window resized: {:?}", physical_size);
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(physical_size);
                }
            }
            WindowEvent::RedrawRequested => {
                // Render frame using current engine state
                if let Some(renderer) = &mut self.renderer {
                    if let Err(e) = renderer.render() {
                        tracing::error!("Render error: {}", e);
                    }
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Initialize renderer if we have window but no renderer
        if self.window.is_some() && self.renderer.is_none() {
            if let Some(window) = &self.window {
                tracing::info!("Initializing renderer...");
                // Block on renderer initialization (this is acceptable for startup)
                match pollster::block_on(SpriteRenderer::new(window.clone())) {
                    Ok(renderer) => {
                        tracing::info!("Renderer initialized successfully");
                        self.renderer = Some(renderer);
                        // Call script on_start once after renderer is ready
                        if !self.script_on_start_called {
                            if let Some(cb) = &mut self.script_on_start {
                                tracing::info!("Calling script on_start()");
                                cb(&mut self.engine_state);
                            }
                            self.script_on_start_called = true;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to initialize renderer: {}", e);
                        return; // Skip this frame
                    }
                }
            }
        }

        // Skip rendering if renderer isn't ready
        if self.renderer.is_none() {
            return;
        }

        // Begin frame metrics collection
        self.metrics.begin_frame();
        self.frame_count += 1;

        // Run fixed timestep updates (invoke script on_update if provided)
        let mut updates_run = 0;
        self.timestep.update(|dt| {
            updates_run += 1;
            // This runs at exactly 60 FPS for deterministic behavior
            if updates_run <= 5 {
                // Limit debug spam
                tracing::debug!("Fixed update: dt={:.6}s", dt);
            }

            // Update engine time
            self.engine_state.update_time(dt);

            // Invoke script with simple watchdog
            if let Some(cb) = &mut self.script_on_update {
                let start = std::time::Instant::now();
                cb(dt, &mut self.engine_state);
                let elapsed = start.elapsed();
                if elapsed.as_micros() > 2_000 {
                    // > 2ms
                    tracing::warn!("Watchdog: on_update took {:?}", elapsed);
                }
            }
        });

        // Update renderer with current engine state
        if let Some(renderer) = &mut self.renderer {
            if let Err(e) = renderer.update_from_engine_state(&self.engine_state) {
                tracing::error!("Failed to update renderer from engine state: {}", e);
            }
        }

        // Reset engine frame counters
        self.engine_state.reset_frame_counters();

        // End frame metrics collection
        self.metrics.end_frame();

        // Log metrics every 5 seconds (300 frames at 60 FPS)
        if self.frame_count % 300 == 0 {
            let stats = self.metrics.get_performance_stats();
            let violations = self.metrics.validate_performance_budgets();
            let engine_violations = self.engine_state.validate_performance_budgets();

            tracing::info!(
                "Performance stats ({}s): CPU mean={:.2}ms, p99={:.2}ms, FFI calls={:.1}",
                self.frame_count / 60,
                stats.get("cpu_frame_mean_ms").unwrap_or(&0.0),
                stats.get("cpu_frame_p99_ms").unwrap_or(&0.0),
                stats.get("ffi_calls_max").unwrap_or(&0.0),
            );

            for violation in violations.iter().chain(engine_violations.iter()) {
                tracing::warn!("Performance budget violation: {}", violation);
            }

            if let Some(renderer) = &self.renderer {
                tracing::info!(
                    "Rendered {} sprites this frame",
                    renderer.get_sprite_count()
                );
            }
        }

        // Request redraw
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
