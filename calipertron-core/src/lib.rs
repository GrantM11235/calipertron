#![no_std]

use core::f32::consts::PI;

use num_traits::Float;
pub struct PhaseAccumulator {
    pub unwrapped_phase: f32,
    last_phase: f32,
    hysteresis_threshold: f32,
}

impl PhaseAccumulator {
    pub fn new(initial_phase: f32, hysteresis_threshold: f32) -> Self {
        PhaseAccumulator {
            unwrapped_phase: 0.0,
            last_phase: initial_phase,
            hysteresis_threshold,
        }
    }

    pub fn update(&mut self, new_phase: f32) {
        let mut delta = new_phase - self.last_phase;

        // Handle wraparound
        if delta > PI {
            delta -= 2.0 * PI;
        } else if delta < -PI {
            delta += 2.0 * PI;
        }

        // Apply hysteresis
        if delta.abs() > self.hysteresis_threshold {
            self.unwrapped_phase += delta;
            self.last_phase = new_phase;
        }
    }
}
