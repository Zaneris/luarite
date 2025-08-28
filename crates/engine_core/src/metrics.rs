use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FrameMetrics {
    pub cpu_frame_ms: f64,
    pub gpu_frame_ms: f64,
    pub draw_calls: u32,
    pub sprites_submitted: u32,
    pub ffi_calls: u32,
    pub rust_allocs_frame: u32,
    pub lua_gc_time_ms: f64,
    pub lua_mem_mb: f64,
    pub log_dropped_count: u32,
    pub watchdog_spikes: u32,
    pub reload_count: u32,
}

impl Default for FrameMetrics {
    fn default() -> Self {
        Self {
            cpu_frame_ms: 0.0,
            gpu_frame_ms: 0.0,
            draw_calls: 0,
            sprites_submitted: 0,
            ffi_calls: 0,
            rust_allocs_frame: 0,
            lua_gc_time_ms: 0.0,
            lua_mem_mb: 0.0,
            log_dropped_count: 0,
            watchdog_spikes: 0,
            reload_count: 0,
        }
    }
}

pub struct MetricsCollector {
    current_frame: FrameMetrics,
    frame_start: Option<Instant>,
    
    // Counters for this frame
    ffi_calls_this_frame: AtomicU32,
    draw_calls_this_frame: AtomicU32,
    sprites_this_frame: AtomicU32,
    
    // Historical data for performance analysis
    frame_history: Vec<FrameMetrics>,
    max_history: usize,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            current_frame: FrameMetrics::default(),
            frame_start: None,
            ffi_calls_this_frame: AtomicU32::new(0),
            draw_calls_this_frame: AtomicU32::new(0),
            sprites_this_frame: AtomicU32::new(0),
            frame_history: Vec::new(),
            max_history: 300, // Keep 5 seconds of history at 60 FPS
        }
    }

    pub fn begin_frame(&mut self) {
        self.frame_start = Some(Instant::now());
        self.ffi_calls_this_frame.store(0, Ordering::Relaxed);
        self.draw_calls_this_frame.store(0, Ordering::Relaxed);
        self.sprites_this_frame.store(0, Ordering::Relaxed);
    }

    pub fn end_frame(&mut self) {
        if let Some(start) = self.frame_start.take() {
            let frame_time = start.elapsed();
            self.current_frame.cpu_frame_ms = frame_time.as_secs_f64() * 1000.0;
        }

        self.current_frame.ffi_calls = self.ffi_calls_this_frame.load(Ordering::Relaxed);
        self.current_frame.draw_calls = self.draw_calls_this_frame.load(Ordering::Relaxed);
        self.current_frame.sprites_submitted = self.sprites_this_frame.load(Ordering::Relaxed);

        // Store frame in history
        self.frame_history.push(self.current_frame.clone());
        if self.frame_history.len() > self.max_history {
            self.frame_history.remove(0);
        }
    }

    pub fn record_ffi_call(&self) {
        self.ffi_calls_this_frame.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_draw_call(&self, sprite_count: u32) {
        self.draw_calls_this_frame.fetch_add(1, Ordering::Relaxed);
        self.sprites_this_frame.fetch_add(sprite_count, Ordering::Relaxed);
    }

    pub fn record_lua_gc(&mut self, duration: Duration, memory_mb: f64) {
        self.current_frame.lua_gc_time_ms = duration.as_secs_f64() * 1000.0;
        self.current_frame.lua_mem_mb = memory_mb;
    }

    pub fn record_watchdog_spike(&mut self) {
        self.current_frame.watchdog_spikes += 1;
    }

    pub fn record_reload(&mut self) {
        self.current_frame.reload_count += 1;
    }

    pub fn current_metrics(&self) -> &FrameMetrics {
        &self.current_frame
    }

    pub fn get_performance_stats(&self) -> HashMap<String, f64> {
        if self.frame_history.is_empty() {
            return HashMap::new();
        }

        let mut stats = HashMap::new();
        let frames: Vec<&FrameMetrics> = self.frame_history.iter().collect();
        
        // Calculate CPU frame time stats
        let cpu_times: Vec<f64> = frames.iter().map(|f| f.cpu_frame_ms).collect();
        stats.insert("cpu_frame_mean_ms".to_string(), mean(&cpu_times));
        stats.insert("cpu_frame_p99_ms".to_string(), percentile(&cpu_times, 0.99));
        stats.insert("cpu_frame_max_ms".to_string(), cpu_times.iter().copied().fold(0.0, f64::max));

        // FFI calls per frame (should be <= 3 per plan)
        let ffi_calls: Vec<f64> = frames.iter().map(|f| f.ffi_calls as f64).collect();
        stats.insert("ffi_calls_mean".to_string(), mean(&ffi_calls));
        stats.insert("ffi_calls_max".to_string(), ffi_calls.iter().copied().fold(0.0, f64::max));

        stats
    }

    pub fn validate_performance_budgets(&self) -> Vec<String> {
        let mut violations = Vec::new();
        let stats = self.get_performance_stats();

        // Check fitness functions from the plan
        if let Some(&p99) = stats.get("cpu_frame_p99_ms") {
            if p99 > 16.6 {
                violations.push(format!("p99_frame_ms ({:.2}) exceeds 16.6ms budget", p99));
            }
        }

        if let Some(&mean) = stats.get("cpu_frame_mean_ms") {
            if mean > 4.0 {
                violations.push(format!("mean_frame_ms ({:.2}) exceeds 4.0ms budget", mean));
            }
        }

        if let Some(&max_ffi) = stats.get("ffi_calls_max") {
            if max_ffi > 3.0 {
                violations.push(format!("ffi_calls_per_frame ({}) exceeds budget of 3", max_ffi as u32));
            }
        }

        violations
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / values.len() as f64 }
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() { return 0.0; }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let index = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}