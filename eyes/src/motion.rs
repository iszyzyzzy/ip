//! Persistent configuration and deterministic motion shared by UI and RPC.
use crate::control::pb;
use serde::{Deserialize, Serialize};

macro_rules! settings {
    ($name:ident, $proto:ident, {$($field:ident: $ty:ty = $default:expr),* $(,)?}) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct $name { $(pub $field: $ty),* }
        impl Default for $name { fn default() -> Self { Self { $($field: $default),* } } }
        impl $name {
            pub fn patch(&mut self, c: pb::$proto) { $(if let Some(v) = c.$field { self.$field = v; })* }
            pub fn proto(&self) -> pb::$proto { pb::$proto { $($field: Some(self.$field)),* } }
        }
    }
}
settings!(Jelly, JellyConfig, { enabled: bool = false, strength: f32 = 0.65, frequency: f32 = 3.5, damping: f32 = 0.3 });
settings!(Layers, LayerConfig, { iris: bool = true, pupil: bool = true, highlights: bool = true, sclera: bool = true, upper_lid: bool = true, lower_lid: bool = true, brow: bool = true });
settings!(Simulator, SimulatorConfig, {
    target_x: f32 = 0., target_y: f32 = 0., eye_motion: bool = true,
    motion_amplitude: f32 = 0.06, motion_speed: f32 = 1.5, convergence: f32 = 0.,
    left_openness: f32 = 1., right_openness: f32 = 1., left_brow: f32 = 0., right_brow: f32 = 0.,
    roll: f32 = 0., show_target: bool = true,
});
settings!(Shape, ArtConfig, {
    half_width: f32 = 0.255, upper_height: f32 = 0.205, lower_height: f32 = 0.205,
    roundness: f32 = 0.85, iris_aspect: f32 = 0.90, brow_height: f32 = 0.30,
    lid_texture_height: f32 = 0.16, shadow_strength: f32 = 0.12,
});
impl Shape {
    pub fn from_art(a: &crate::assets::Art) -> Self {
        Self {
            half_width: a.half_width,
            upper_height: a.upper_height,
            lower_height: a.lower_height,
            roundness: a.roundness,
            iris_aspect: a.iris_aspect,
            brow_height: a.brow_height,
            lid_texture_height: a.lid_texture_height,
            shadow_strength: a.shadow_strength,
        }
    }
    pub fn uniforms(&self) -> [[f32; 4]; 2] {
        [
            [
                self.half_width,
                self.upper_height,
                self.lower_height,
                self.roundness,
            ],
            [
                self.iris_aspect,
                self.brow_height,
                self.lid_texture_height,
                self.shadow_strength,
            ],
        ]
    }
}
impl Layers {
    pub fn uniforms(&self) -> [[f32; 4]; 2] {
        let f = |b| if b { 1. } else { 0. };
        [
            [
                f(self.iris),
                f(self.pupil),
                f(self.highlights),
                f(self.sclera),
            ],
            [f(self.upper_lid), f(self.lower_lid), f(self.brow), 0.],
        ]
    }
}
impl Simulator {
    pub fn pose(&self, t: f32) -> [[f32; 4]; 2] {
        let mut motion = [0.; 2];
        if self.eye_motion {
            let phase = t * self.motion_speed;
            let n = phase.floor();
            let f = (phase.fract() * 8.).clamp(0., 1.);
            let blend = f * f * (3. - 2. * f);
            for (axis, v) in motion.iter_mut().enumerate() {
                let sample = |i: f32| ((i + 1.) * (2.399 + axis as f32 * 1.73)).sin();
                *v = (sample(n - 1.) * (1. - blend) + sample(n) * blend) * self.motion_amplitude;
            }
        }
        let x = self.target_x + motion[0];
        let y = (self.target_y + motion[1]).clamp(-1., 1.);
        [
            [
                (x - self.convergence).clamp(-1., 1.),
                y,
                self.left_openness,
                self.left_brow,
            ],
            [
                (x + self.convergence).clamp(-1., 1.),
                y,
                self.right_openness,
                self.right_brow,
            ],
        ]
    }
}

#[derive(Default)]
pub struct Spring {
    position: [f32; 3],
    velocity: [f32; 3],
    initialized: bool,
}
impl Spring {
    /// Damped spring with bounded substeps; disabling resets state immediately.
    pub fn step(&mut self, eye: [f32; 4], cfg: &Jelly, dt: f32) -> [f32; 4] {
        let target = [eye[0], eye[1], eye[2]];
        if !self.initialized || !cfg.enabled || cfg.strength == 0. {
            self.position = target;
            self.velocity = [0.; 3];
            self.initialized = true;
            return [0.; 4];
        }
        let dt = dt.clamp(0., 0.1);
        let steps = (dt * 240.).ceil().max(1.) as usize;
        let h = dt / steps as f32;
        let omega = std::f32::consts::TAU * cfg.frequency;
        for _ in 0..steps {
            for (i, value) in target.iter().enumerate() {
                let acc = omega * omega * (value - self.position[i])
                    - 2. * cfg.damping * omega * self.velocity[i];
                self.velocity[i] += acc * h;
                self.position[i] += self.velocity[i] * h;
            }
        }
        let x = (self.position[0] - target[0]).clamp(-1., 1.);
        let y = (self.position[1] - target[1]).clamp(-1., 1.);
        let squash =
            ((self.position[2] - target[2]) * 0.24 + self.velocity[2] * 0.008).clamp(-0.22, 0.22);
        [
            x * 0.065 * cfg.strength,
            y * 0.045 * cfg.strength,
            squash * cfg.strength,
            x * 0.12 * cfg.strength,
        ]
    }
}

pub struct Blinker {
    start: [f32; 2],
    duration: [f32; 2],
    next: f32,
    was_auto: bool,
}
impl Default for Blinker {
    fn default() -> Self {
        Self {
            start: [-10.; 2],
            duration: [0.24; 2],
            next: 0.,
            was_auto: false,
        }
    }
}
impl Blinker {
    pub fn trigger(&mut self, t: f32, eyes: [bool; 2], duration: f32) {
        for (i, enabled) in eyes.iter().enumerate() {
            if *enabled {
                self.start[i] = t;
                self.duration[i] = duration;
            }
        }
    }
    pub fn reschedule(&mut self) {
        self.was_auto = false;
    }
    pub fn apply(
        &mut self,
        pose: &mut [[f32; 4]; 2],
        t: f32,
        auto: bool,
        interval: f32,
        duration: f32,
        variation: f32,
    ) {
        if auto && !self.was_auto {
            self.next = t + interval;
        }
        self.was_auto = auto;
        if auto && t >= self.next {
            self.trigger(t, [true; 2], duration);
            self.next = t + interval * (1. + variation * (t * 1.713).sin());
        }
        for (i, eye) in pose.iter_mut().enumerate() {
            let phase = (t - self.start[i]) / self.duration[i];
            if (0.0..1.0).contains(&phase) {
                // Brief closed plateau makes even the shortest blink visible.
                let closed = if phase < 0.35 {
                    phase / 0.35
                } else if phase < 0.55 {
                    1.
                } else {
                    (1. - phase) / 0.45
                };
                eye[2] *= 1. - closed;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_and_patch_are_exact_without_saccades() {
        let mut s = Simulator::default();
        s.patch(pb::SimulatorConfig {
            target_x: Some(0.7),
            target_y: Some(-0.4),
            eye_motion: Some(false),
            ..Default::default()
        });
        s.patch(pb::SimulatorConfig {
            target_x: Some(-0.8),
            ..Default::default()
        });
        assert_eq!(s.pose(99.)[0], [-0.8, -0.4, 1., 0.]);
        s.eye_motion = true;
        assert_ne!(s.pose(1.), s.pose(2.));
        let restored: Simulator =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(restored, s);
    }
    #[test]
    fn spring_rebounds_settles_and_disables() {
        let mut spring = Spring::default();
        let mut cfg = Jelly {
            enabled: true,
            ..Default::default()
        };
        spring.step([0., 0., 1., 0.], &cfg, 0.016);
        let first = spring.step([1., 0., 1., 0.], &cfg, 0.016);
        assert!(first[0] < -0.01);
        let mut overshoot = false;
        for _ in 0..120 {
            let d = spring.step([1., 0., 1., 0.], &cfg, 0.016);
            overshoot |= d[0] > 0.001;
        }
        assert!(overshoot);
        assert!(spring.step([1., 0., 1., 0.], &cfg, 0.016)[0].abs() < 0.001);
        cfg.enabled = false;
        assert_eq!(spring.step([-1., 0., 0., 0.], &cfg, 0.016), [0.; 4]);
    }
    #[test]
    fn independent_wink_and_auto_toggle() {
        let mut b = Blinker::default();
        b.trigger(0., [true, false], 0.3);
        let mut p = [[0., 0., 1., 0.]; 2];
        b.apply(&mut p, 0.14, false, 4., 0.3, 0.);
        assert_eq!(p[0][2], 0.);
        assert_eq!(p[1][2], 1.);
        b.apply(&mut p, 1., true, 2., 0.3, 0.);
        let mut p = [[0., 0., 1., 0.]; 2];
        b.apply(&mut p, 3., true, 2., 0.3, 0.);
        b.apply(&mut p, 3.15, true, 2., 0.3, 0.);
        assert_eq!(p[0][2], 0.);
        assert_eq!(p[1][2], 0.);
    }
}
