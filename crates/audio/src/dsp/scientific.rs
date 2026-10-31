//! Premiere's Scientific Filter: Bessel, Butterworth and Chebyshev designs up to eighth
//! order as low, high, band pass or band stop, mapped from an analog prototype by the
//! bilinear transform onto a cascade of second-order sections.

use std::f64::consts::PI;

use super::biquad::{Biquad, Coefficients};
use crate::SampleRate;

const MAX_FILTER_ORDER: usize = 8;
/// Band filters double the prototype's order, so carry twice its roots.
const MAX_ROOTS: usize = 2 * MAX_FILTER_ORDER;
const MAX_SECTIONS: usize = MAX_FILTER_ORDER;
const LOWEST_HZ: f64 = 10.0;
/// Roots this close to the real axis are taken as real, against rounding.
const REAL_TOLERANCE: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterDesign {
    /// The flattest delay, so transients keep their shape; the gentlest slope.
    Bessel,
    /// The flattest passband.
    Butterworth,
    /// A steeper slope bought with ripple in the passband.
    Chebyshev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterBand {
    LowPass,
    HighPass,
    BandPass,
    BandStop,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScientificSettings {
    pub design: FilterDesign,
    pub band: FilterBand,
    /// 1 to 8; each order steepens the slope by 6 dB an octave.
    pub order: usize,
    /// Hertz; the band's lower edge for the band modes.
    pub cutoff: f32,
    /// Hertz; the band's upper edge, read only by the band modes.
    pub high_cutoff: f32,
    /// Decibels the Chebyshev passband ripples by.
    pub ripple_db: f32,
    pub gain_db: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    fn real(re: f64) -> Self {
        Self::new(re, 0.0)
    }

    fn add(self, other: Self) -> Self {
        Self::new(self.re + other.re, self.im + other.im)
    }

    fn sub(self, other: Self) -> Self {
        Self::new(self.re - other.re, self.im - other.im)
    }

    fn mul(self, other: Self) -> Self {
        Self::new(
            self.re * other.re - self.im * other.im,
            self.re * other.im + self.im * other.re,
        )
    }

    fn scale(self, factor: f64) -> Self {
        Self::new(self.re * factor, self.im * factor)
    }

    fn div(self, other: Self) -> Self {
        let norm = other.re * other.re + other.im * other.im;
        Self::new(
            (self.re * other.re + self.im * other.im) / norm,
            (self.im * other.re - self.re * other.im) / norm,
        )
    }

    fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    fn sqrt(self) -> Self {
        let root = self.abs().sqrt();
        let half = self.im.atan2(self.re) / 2.0;
        Self::new(root * half.cos(), root * half.sin())
    }
}

/// A fixed-capacity list of roots, so designing allocates nothing.
#[derive(Clone, Copy)]
struct Roots {
    values: [Complex; MAX_ROOTS],
    len: usize,
}

impl Roots {
    const EMPTY: Self = Self {
        values: [Complex::new(0.0, 0.0); MAX_ROOTS],
        len: 0,
    };

    fn push(&mut self, root: Complex) {
        if let Some(slot) = self.values.get_mut(self.len) {
            *slot = root;
            self.len += 1;
        }
    }

    fn iter(&self) -> impl Iterator<Item = Complex> + '_ {
        self.values[..self.len].iter().copied()
    }

    fn repeated(root: Complex, count: usize) -> Self {
        let mut roots = Self::EMPTY;
        (0..count).for_each(|_| roots.push(root));
        roots
    }
}

/// The prototype's poles for a cutoff of one radian a second, and its gain at zero
/// frequency.
fn prototype(design: FilterDesign, order: usize, ripple_db: f32) -> (Roots, f64) {
    let n = order as f64;
    let mut poles = Roots::EMPTY;
    match design {
        FilterDesign::Butterworth => {
            for k in 0..order {
                let angle = PI * (2.0 * k as f64 + n + 1.0) / (2.0 * n);
                poles.push(Complex::new(angle.cos(), angle.sin()));
            }
            (poles, 1.0)
        }
        FilterDesign::Chebyshev => {
            let ripple = f64::from(ripple_db.max(0.01));
            let epsilon = (10.0_f64.powf(ripple / 10.0) - 1.0).sqrt();
            let mu = (1.0 / epsilon).asinh() / n;
            for k in 0..order {
                let angle = PI * (2.0 * k as f64 + 1.0) / (2.0 * n);
                poles.push(Complex::new(
                    -mu.sinh() * angle.sin(),
                    mu.cosh() * angle.cos(),
                ));
            }
            // Even orders start the passband at a trough of the ripple.
            let dc = match order % 2 {
                0 => 1.0 / (1.0 + epsilon * epsilon).sqrt(),
                _ => 1.0,
            };
            (poles, dc)
        }
        FilterDesign::Bessel => (bessel_poles(order), 1.0),
    }
}

/// The reverse Bessel polynomial's roots, scaled so the response is 3 dB down at one
/// radian a second like the other designs.
fn bessel_poles(order: usize) -> Roots {
    let factorial = |k: usize| (1..=k).map(|i| i as f64).product::<f64>();
    let mut coefficients = [0.0; MAX_FILTER_ORDER + 1];
    for (k, coefficient) in coefficients.iter_mut().enumerate().take(order + 1) {
        *coefficient = factorial(2 * order - k)
            / (2.0_f64.powi((order - k) as i32) * factorial(k) * factorial(order - k));
    }
    let polynomial = |s: Complex| {
        coefficients[..=order]
            .iter()
            .rev()
            .fold(Complex::default(), |sum, &c| {
                sum.mul(s).add(Complex::real(c))
            })
    };
    // Durand–Kerner: every root at once, each refined against the others.
    let mut roots = Roots::EMPTY;
    let seed = Complex::new(0.4, 0.9);
    let mut guess = Complex::real(1.0);
    for _ in 0..order {
        roots.push(guess);
        guess = guess.mul(seed);
    }
    for _ in 0..500 {
        for i in 0..order {
            let root = roots.values[i];
            let others = (0..order)
                .filter(|&j| j != i)
                .fold(Complex::real(1.0), |product, j| {
                    product.mul(root.sub(roots.values[j]))
                });
            roots.values[i] = root.sub(polynomial(root).div(others));
        }
    }
    for root in &mut roots.values[..order] {
        if root.im.abs() < REAL_TOLERANCE {
            root.im = 0.0;
        }
    }
    let level = |w: f64| {
        let s = Complex::new(0.0, w);
        let denominator = roots
            .iter()
            .fold(Complex::real(1.0), |product, pole| product.mul(s.sub(pole)));
        coefficients[0] / denominator.abs()
    };
    let half_power = std::f64::consts::FRAC_1_SQRT_2;
    let (mut low, mut high) = (1e-3_f64, 1e3_f64);
    for _ in 0..100 {
        let middle = (low * high).sqrt();
        match level(middle) > half_power {
            true => low = middle,
            false => high = middle,
        }
    }
    roots.values[..order]
        .iter_mut()
        .for_each(|root| *root = root.scale(1.0 / low));
    roots
}

/// The z-plane root an s-plane root maps to.
fn bilinear(root: Complex, twice_rate: f64) -> Complex {
    Complex::real(twice_rate)
        .add(root)
        .div(Complex::real(twice_rate).sub(root))
}

/// The second-order factors, in powers of 1/z, of a polynomial with conjugate-paired
/// `roots`; a lone real root makes a first-order factor.
fn quadratics(roots: &Roots) -> ([[f64; 3]; MAX_SECTIONS], usize) {
    let mut factors = [[0.0; 3]; MAX_SECTIONS];
    let mut count = 0;
    let mut push = |factor: [f64; 3]| {
        if let Some(slot) = factors.get_mut(count) {
            *slot = factor;
            count += 1;
        }
    };
    let mut lone_real: Option<f64> = None;
    for root in roots.iter() {
        if root.im > REAL_TOLERANCE {
            push([1.0, -2.0 * root.re, root.re * root.re + root.im * root.im]);
        } else if root.im.abs() <= REAL_TOLERANCE {
            match lone_real.take() {
                Some(other) => push([1.0, -(root.re + other), root.re * other]),
                None => lone_real = Some(root.re),
            }
        }
    }
    if let Some(real) = lone_real {
        push([1.0, -real, 0.0]);
    }
    (factors, count)
}

/// How much `b / a` scales a sine at `w` radians a sample.
fn response(b: [f64; 3], a: [f64; 3], w: f64) -> f64 {
    let z = Complex::new(w.cos(), -w.sin());
    let evaluate = |c: [f64; 3]| {
        Complex::real(c[0])
            .add(z.scale(c[1]))
            .add(z.mul(z).scale(c[2]))
    };
    evaluate(b).abs() / evaluate(a).abs()
}

/// Designs the sections `settings` ask for at `rate`: their coefficients and how many.
fn design(
    rate: SampleRate,
    settings: &ScientificSettings,
) -> ([Coefficients; MAX_SECTIONS], usize) {
    let fs = f64::from(rate.0);
    let twice_rate = 2.0 * fs;
    let warped = |hz: f32| {
        let hz = f64::from(hz).clamp(LOWEST_HZ, fs * 0.49);
        twice_rate * (PI * hz / fs).tan()
    };
    let order = settings.order.clamp(1, MAX_FILTER_ORDER);
    let (prototype, dc) = prototype(settings.design, order, settings.ripple_db);
    let low = warped(settings.cutoff);
    let high = warped(settings.high_cutoff).max(low * 1.01);
    let center = (low * high).sqrt();
    let width = high - low;
    let mut poles = Roots::EMPTY;
    let (zeros, reference) = match settings.band {
        FilterBand::LowPass => {
            prototype.iter().for_each(|p| poles.push(p.scale(low)));
            (Roots::repeated(Complex::real(-1.0), order), 0.0)
        }
        FilterBand::HighPass => {
            let corner = Complex::real(low);
            prototype.iter().for_each(|p| poles.push(corner.div(p)));
            (Roots::repeated(Complex::real(1.0), order), PI)
        }
        FilterBand::BandPass | FilterBand::BandStop => {
            let square = Complex::real(center * center);
            for p in prototype.iter() {
                let half = match settings.band {
                    FilterBand::BandPass => p.scale(width / 2.0),
                    _ => Complex::real(width / 2.0).div(p),
                };
                let root = half.mul(half).sub(square).sqrt();
                poles.push(half.add(root));
                poles.push(half.sub(root));
            }
            let mut zeros = Roots::EMPTY;
            match settings.band {
                FilterBand::BandPass => {
                    (0..order).for_each(|_| zeros.push(Complex::real(1.0)));
                    (0..order).for_each(|_| zeros.push(Complex::real(-1.0)));
                    let middle = 2.0 * (center / twice_rate).atan();
                    (zeros, middle)
                }
                _ => {
                    let notch = bilinear(Complex::new(0.0, center), twice_rate);
                    for _ in 0..order {
                        zeros.push(notch);
                        zeros.push(Complex::new(notch.re, -notch.im));
                    }
                    (zeros, 0.0)
                }
            }
        }
    };
    let mut digital = Roots::EMPTY;
    poles
        .iter()
        .for_each(|pole| digital.push(bilinear(pole, twice_rate)));
    let (denominators, count) = quadratics(&digital);
    let (mut numerators, _) = quadratics(&zeros);
    let level: f64 = (0..count)
        .map(|index| response(numerators[index], denominators[index], reference))
        .product();
    let gain = 10.0_f64.powf(f64::from(settings.gain_db) / 20.0);
    let scale = dc / level.max(f64::MIN_POSITIVE) * gain;
    numerators[0] = numerators[0].map(|c| c * scale);
    let mut sections = [Coefficients::IDENTITY; MAX_SECTIONS];
    for (section, (b, a)) in sections
        .iter_mut()
        .zip(numerators.iter().zip(&denominators))
        .take(count)
    {
        *section = Coefficients::normalised(*b, *a);
    }
    (sections, count)
}

/// A Scientific Filter over interleaved stereo.
#[derive(Debug, Clone, Default)]
pub struct ScientificFilter {
    sections: [Biquad; MAX_SECTIONS],
    count: usize,
    designed: Option<(SampleRate, ScientificSettings)>,
}

impl ScientificFilter {
    pub fn reset(&mut self) {
        self.sections.iter_mut().for_each(Biquad::reset);
    }

    pub fn process(
        &mut self,
        settings: &ScientificSettings,
        rate: SampleRate,
        samples: &mut [f32],
    ) {
        if self.designed != Some((rate, *settings)) {
            let reshaped = self.designed.is_none_or(|(_, old)| {
                (old.design, old.band, old.order)
                    != (settings.design, settings.band, settings.order)
            });
            if reshaped {
                self.reset();
            }
            let (coefficients, count) = design(rate, settings);
            for (section, coefficients) in self.sections.iter_mut().zip(coefficients) {
                section.set(coefficients);
            }
            self.count = count;
            self.designed = Some((rate, *settings));
        }
        for section in &mut self.sections[..self.count] {
            section.process(samples);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: SampleRate = SampleRate(48_000);

    fn settings(design: FilterDesign, band: FilterBand, order: usize) -> ScientificSettings {
        ScientificSettings {
            design,
            band,
            order,
            cutoff: 1_000.0,
            high_cutoff: 4_000.0,
            ripple_db: 1.0,
            gain_db: 0.0,
        }
    }

    fn db(settings: &ScientificSettings, hz: f32) -> f32 {
        let (sections, count) = design(RATE, settings);
        let level: f32 = sections[..count]
            .iter()
            .map(|section| section.magnitude(RATE, hz))
            .product();
        20.0 * level.log10()
    }

    #[test]
    fn each_design_meets_its_cutoff_and_slope() {
        use FilterBand::*;
        use FilterDesign::*;
        let butterworth = settings(Butterworth, LowPass, 4);
        assert!(db(&butterworth, 20.0).abs() < 0.01);
        assert!((db(&butterworth, 1_000.0) + 3.01).abs() < 0.05);
        assert!(db(&butterworth, 4_000.0) < -45.0);
        let chebyshev = settings(Chebyshev, LowPass, 4);
        assert!((db(&chebyshev, 20.0) + 1.0).abs() < 0.05);
        assert!((db(&chebyshev, 1_000.0) + 1.0).abs() < 0.05);
        assert!(db(&chebyshev, 4_000.0) < db(&butterworth, 4_000.0) - 10.0);
        let bessel = settings(Bessel, LowPass, 4);
        assert!(db(&bessel, 20.0).abs() < 0.01);
        assert!((db(&bessel, 1_000.0) + 3.01).abs() < 0.05);
        let high = settings(Butterworth, HighPass, 3);
        assert!(db(&high, 23_000.0).abs() < 0.05);
        assert!((db(&high, 1_000.0) + 3.01).abs() < 0.05);
        assert!(db(&high, 250.0) < -35.0);
    }

    #[test]
    fn band_modes_pass_or_stop_between_their_edges() {
        let pass = settings(FilterDesign::Butterworth, FilterBand::BandPass, 2);
        assert!(db(&pass, 2_000.0).abs() < 0.05);
        assert!((db(&pass, 1_000.0) + 3.01).abs() < 0.05);
        assert!((db(&pass, 4_000.0) + 3.01).abs() < 0.05);
        assert!(db(&pass, 100.0) < -35.0);
        let stop = settings(FilterDesign::Butterworth, FilterBand::BandStop, 2);
        assert!(db(&stop, 2_000.0) < -60.0);
        assert!(db(&stop, 20.0).abs() < 0.05);
        assert!(db(&stop, 20_000.0).abs() < 0.2);
    }
}
