use audio::{distort, widen, Biquad, Coefficients, Limiter, Reverb, SampleRate};

/// Where the Mastering equalizer's three bands sit.
const LOW_SHELF_HZ: f32 = 100.0;
const MID_HZ: f32 = 1_000.0;
const MID_Q: f32 = 0.7;
const HIGH_SHELF_HZ: f32 = 8_000.0;
/// A mastering reverb only glues the mix together, so even full amount stays a touch.
const REVERB_MIX_AT_FULL: f32 = 0.3;
const REVERB_SIZE: f32 = 0.4;
const REVERB_DAMPING: f32 = 0.6;
/// The exciter saturates only what is above this, adding harmonics to the top end.
const EXCITER_HZ: f32 = 3_000.0;
const EXCITER_DRIVE_DB: f32 = 18.0;
const EXCITER_MIX_AT_FULL: f32 = 0.5;
/// How much the loudness maximizer pushes into its ceiling at full amount.
const MAXIMIZER_BOOST_DB: f32 = 12.0;
const MAXIMIZER_CEILING_DB: f32 = -0.3;
const MAXIMIZER_RELEASE_MS: f32 = 100.0;

/// Settings for one block, as decibels and fractions.
pub(crate) struct MasteringSettings {
    pub(crate) low_db: f32,
    pub(crate) mid_db: f32,
    pub(crate) high_db: f32,
    pub(crate) reverb: f32,
    pub(crate) exciter: f32,
    pub(crate) width: f32,
    pub(crate) maximizer: f32,
    pub(crate) output_db: f32,
}

/// Premiere's Mastering: an equalizer, a touch of reverb, an exciter, a widener and a
/// loudness maximizer in the order a mastering engineer would chain them.
pub(crate) struct MasteringChain {
    tone: [Biquad; 3],
    reverb: Reverb,
    exciter_split: Biquad,
    /// The highs the exciter works on, kept between blocks so it is allocated once.
    highs: Vec<f32>,
    maximizer: Limiter,
}

impl MasteringChain {
    pub(crate) fn new(rate: SampleRate) -> Self {
        Self {
            tone: [Biquad::default(); 3],
            reverb: Reverb::new(rate),
            exciter_split: Biquad::default(),
            highs: Vec::new(),
            maximizer: Limiter::default(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.tone.iter_mut().for_each(Biquad::reset);
        self.reverb.reset();
        self.exciter_split.reset();
        self.maximizer.reset();
    }

    pub(crate) fn process(
        &mut self,
        settings: &MasteringSettings,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        let shapes = [
            Coefficients::low_shelf(rate, LOW_SHELF_HZ, settings.low_db),
            Coefficients::peaking(rate, MID_HZ, MID_Q, settings.mid_db),
            Coefficients::high_shelf(rate, HIGH_SHELF_HZ, settings.high_db),
        ];
        for (filter, shape) in self.tone.iter_mut().zip(shapes) {
            filter.set(shape);
            filter.process(samples);
        }
        if settings.reverb > 0.0 {
            let mix = settings.reverb * REVERB_MIX_AT_FULL;
            self.reverb
                .process(REVERB_SIZE, REVERB_DAMPING, mix, samples);
        }
        if settings.exciter > 0.0 {
            self.excite(settings.exciter, rate, samples);
        }
        widen(settings.width, samples);
        if settings.maximizer > 0.0 {
            let boost = settings.maximizer * MAXIMIZER_BOOST_DB;
            let ceiling = MAXIMIZER_CEILING_DB;
            self.maximizer
                .process(boost, ceiling, MAXIMIZER_RELEASE_MS, rate, samples);
        }
        let output = 10.0_f32.powf(settings.output_db / 20.0);
        samples.iter_mut().for_each(|sample| *sample *= output);
    }

    /// Adds the saturated highs back on top of the sound, by `amount`.
    fn excite(&mut self, amount: f32, rate: SampleRate, samples: &mut [f32]) {
        self.highs.clear();
        self.highs.extend_from_slice(samples);
        self.exciter_split
            .set(Coefficients::high_pass(rate, EXCITER_HZ, 0.707));
        self.exciter_split.process(&mut self.highs);
        distort(EXCITER_DRIVE_DB, 1.0, &mut self.highs);
        let level = amount * EXCITER_MIX_AT_FULL;
        for (sample, high) in samples.iter_mut().zip(&self.highs) {
            *sample += high * level;
        }
    }
}
