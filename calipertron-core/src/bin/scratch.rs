use calipertron_core::*;
use core::f32::consts::PI;

pub fn main() {
    let mut accumulator = PositionAccumulator::new(0.0, 0.1);
    for position in 0..100 {
        let angle = (position as f32 * 0.1 * PI + PI) % (2.0 * PI) - PI;
        accumulator.update(angle);
        let position = accumulator.get_position();
        println!("Position: {}", position);
    }
}
