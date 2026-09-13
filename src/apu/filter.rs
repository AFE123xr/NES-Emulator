use std::f32::consts::PI;

pub struct FirstOrderFilter {
    alpha: f32,
    prev_input: f32,
    prev_output: f32,
    is_high_pass: bool,
}

impl FirstOrderFilter {
    pub fn low_pass(sample_rate: f32, cutoff_hz: f32) -> Self {
        let dt = 1.0 / sample_rate;
        let rc = 1.0 / (2.0 * PI * cutoff_hz);
        let alpha = dt / (rc + dt);
        Self {
            alpha,
            prev_input: 0.0,
            prev_output: 0.0,
            is_high_pass: false,
        }
    }

    pub fn high_pass(sample_rate: f32, cutoff_hz: f32) -> Self {
        let dt = 1.0 / sample_rate;
        let rc = 1.0 / (2.0 * PI * cutoff_hz);
        let alpha = rc / (rc + dt);
        Self {
            alpha,
            prev_input: 0.0,
            prev_output: 0.0,
            is_high_pass: true,
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let output = if self.is_high_pass {
            self.alpha * (self.prev_output + sample - self.prev_input)
        } else {
            self.prev_output + self.alpha * (sample - self.prev_output)
        };
        self.prev_input = sample;
        self.prev_output = output;
        output
    }
}
