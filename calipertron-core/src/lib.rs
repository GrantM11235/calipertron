#![no_std]

use core::f32::consts::PI;

use num_traits::Float;
pub struct PositionAccumulator {
    pub position: f32,
    last_angle: f32,
    hysteresis_threshold: f32,
}

impl PositionAccumulator {
    pub fn new(initial_angle: f32, hysteresis_threshold: f32) -> Self {
        PositionAccumulator {
            position: 0.0,
            last_angle: initial_angle,
            hysteresis_threshold,
        }
    }

    pub fn update(&mut self, new_angle: f32) {
        let mut delta = new_angle - self.last_angle;

        // Handle wraparound
        if delta > PI {
            delta -= 2.0 * PI;
        } else if delta < -PI {
            delta += 2.0 * PI;
        }

        // Apply hysteresis
        if delta.abs() > self.hysteresis_threshold {
            self.position += delta;
            self.last_angle = new_angle;
        }
    }
}
