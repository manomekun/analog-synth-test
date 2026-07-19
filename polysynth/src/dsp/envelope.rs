//! Analog-style ADSR envelope with exponential segments, based on the classic
//! one-pole-towards-an-overshooting-target formulation (EarLevel Engineering).

/// How far the attack segment overshoots 1.0. Larger values make the attack
/// curve more linear, smaller values more convex.
const ATTACK_TARGET_RATIO: f32 = 0.3;
/// Target offset for decay/release. Small values give the sharp exponential
/// knee typical of analog envelope generators.
const DECAY_TARGET_RATIO: f32 = 0.0001;
/// Length of the fade applied when a voice is stolen, in seconds.
const FAST_RELEASE_TIME: f32 = 0.003;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeState {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
    /// Quick linear fade used when the voice is stolen, to avoid clicks.
    FastRelease,
}

#[derive(Debug, Clone)]
pub struct Envelope {
    sample_rate: f32,
    state: EnvelopeState,
    output: f32,

    attack_coef: f32,
    attack_base: f32,
    decay_coef: f32,
    decay_base: f32,
    sustain_level: f32,
    release_coef: f32,
    release_base: f32,
    fast_release_step: f32,
}

/// One-pole coefficient that traverses a segment in `time_s` seconds when
/// aiming `ratio` beyond the segment's end value.
fn segment_coef(time_s: f32, ratio: f32, sample_rate: f32) -> f32 {
    let samples = time_s * sample_rate;
    if samples < 1.0 {
        0.0
    } else {
        (-(((1.0 + ratio) / ratio).ln()) / samples).exp()
    }
}

impl Envelope {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            state: EnvelopeState::Idle,
            output: 0.0,
            attack_coef: 0.0,
            attack_base: 1.0 + ATTACK_TARGET_RATIO,
            decay_coef: 0.0,
            decay_base: 0.0,
            sustain_level: 1.0,
            release_coef: 0.0,
            release_base: 0.0,
            fast_release_step: 1.0,
        }
    }

    /// Start (or restart) the envelope. The attack always continues from the
    /// current output level so retriggering never produces a click.
    pub fn note_on(&mut self, attack_s: f32, decay_s: f32, sustain: f32, release_s: f32) {
        let sustain = sustain.clamp(0.0, 1.0);

        self.attack_coef = segment_coef(attack_s, ATTACK_TARGET_RATIO, self.sample_rate);
        self.attack_base = (1.0 + ATTACK_TARGET_RATIO) * (1.0 - self.attack_coef);

        self.decay_coef = segment_coef(decay_s, DECAY_TARGET_RATIO, self.sample_rate);
        self.decay_base = (sustain - DECAY_TARGET_RATIO) * (1.0 - self.decay_coef);
        self.sustain_level = sustain;

        self.release_coef = segment_coef(release_s, DECAY_TARGET_RATIO, self.sample_rate);
        self.release_base = -DECAY_TARGET_RATIO * (1.0 - self.release_coef);

        self.state = EnvelopeState::Attack;
    }

    pub fn note_off(&mut self) {
        if self.state != EnvelopeState::Idle {
            self.state = EnvelopeState::Release;
        }
    }

    /// Begin the fast anti-click fade used when this voice is being stolen.
    pub fn fast_release(&mut self) {
        self.fast_release_step = 1.0 / (FAST_RELEASE_TIME * self.sample_rate).max(1.0);
        self.state = EnvelopeState::FastRelease;
    }

    pub fn state(&self) -> EnvelopeState {
        self.state
    }

    pub fn is_finished(&self) -> bool {
        self.state == EnvelopeState::Idle
    }

    pub fn is_releasing(&self) -> bool {
        matches!(
            self.state,
            EnvelopeState::Release | EnvelopeState::FastRelease
        )
    }

    pub fn value(&self) -> f32 {
        self.output
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        match self.state {
            EnvelopeState::Idle => {}
            EnvelopeState::Attack => {
                self.output = self.attack_base + self.output * self.attack_coef;
                if self.output >= 1.0 || self.attack_coef == 0.0 {
                    self.output = 1.0;
                    self.state = EnvelopeState::Decay;
                }
            }
            EnvelopeState::Decay => {
                self.output = self.decay_base + self.output * self.decay_coef;
                if self.output <= self.sustain_level + 1e-5 || self.decay_coef == 0.0 {
                    self.output = self.sustain_level;
                    self.state = EnvelopeState::Sustain;
                }
            }
            EnvelopeState::Sustain => {
                self.output = self.sustain_level;
            }
            EnvelopeState::Release => {
                self.output = self.release_base + self.output * self.release_coef;
                if self.output <= 1e-5 || self.release_coef == 0.0 {
                    self.output = 0.0;
                    self.state = EnvelopeState::Idle;
                }
            }
            EnvelopeState::FastRelease => {
                self.output -= self.fast_release_step;
                if self.output <= 0.0 {
                    self.output = 0.0;
                    self.state = EnvelopeState::Idle;
                }
            }
        }

        self.output
    }
}
