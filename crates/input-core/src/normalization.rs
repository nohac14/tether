use crate::{MAX_ABSOLUTE_FIXED_DELTA, PointerDelta, ScrollAxis, ScrollDelta};
use std::fmt;

const GAIN_SCALE: i64 = 1_000;
const COMBINED_GAIN_SCALE: i64 = GAIN_SCALE * GAIN_SCALE;

/// Smallest supported sensitivity multiplier, expressed in thousandths.
pub const MIN_SENSITIVITY_MILLI: u16 = 125;

/// Largest supported sensitivity multiplier, expressed in thousandths.
pub const MAX_SENSITIVITY_MILLI: u16 = 8_000;

/// Largest pointer acceleration multiplier, expressed in thousandths.
pub const MAX_ACCELERATION_MILLI: u16 = 4_000;

/// Configuration error for deterministic normalization and native translation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalizationConfigError {
    SensitivityOutOfRange,
    AccelerationThresholdOutOfRange,
    AccelerationMultiplierOutOfRange,
    OutputLimitOutOfRange,
    NativeQuantumIsZero,
}

impl fmt::Display for NormalizationConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SensitivityOutOfRange => "sensitivity is outside the supported range",
            Self::AccelerationThresholdOutOfRange => {
                "pointer acceleration threshold is outside the supported range"
            }
            Self::AccelerationMultiplierOutOfRange => {
                "pointer acceleration multiplier is outside the supported range"
            }
            Self::OutputLimitOutOfRange => "normalization output limit is outside the domain",
            Self::NativeQuantumIsZero => "native translation quantum must be non-zero",
        })
    }
}

impl std::error::Error for NormalizationConfigError {}

/// Validated sensitivity multiplier in thousandths (`1000` is 1×).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sensitivity(u16);

impl Sensitivity {
    /// Creates a sensitivity between 0.125× and 8×.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizationConfigError::SensitivityOutOfRange`] outside the
    /// supported range.
    pub const fn from_milli(milli: u16) -> Result<Self, NormalizationConfigError> {
        if milli < MIN_SENSITIVITY_MILLI || milli > MAX_SENSITIVITY_MILLI {
            Err(NormalizationConfigError::SensitivityOutOfRange)
        } else {
            Ok(Self(milli))
        }
    }

    /// Returns a neutral 1× multiplier.
    #[must_use]
    pub const fn unity() -> Self {
        Self(1_000)
    }

    /// Returns the multiplier in thousandths.
    #[must_use]
    pub const fn milli(self) -> u16 {
        self.0
    }
}

impl Default for Sensitivity {
    fn default() -> Self {
        Self::unity()
    }
}

/// Validated absolute bound applied independently to each normalized axis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalizationLimit(i32);

impl NormalizationLimit {
    /// Creates a positive limit no larger than the input domain hard bound.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizationConfigError::OutputLimitOutOfRange`] for zero,
    /// negative, or out-of-domain values.
    pub const fn new(limit: i32) -> Result<Self, NormalizationConfigError> {
        if limit <= 0 || limit > MAX_ABSOLUTE_FIXED_DELTA {
            Err(NormalizationConfigError::OutputLimitOutOfRange)
        } else {
            Ok(Self(limit))
        }
    }

    /// Returns the fixed-point axis bound.
    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }
}

impl Default for NormalizationLimit {
    fn default() -> Self {
        Self(MAX_ABSOLUTE_FIXED_DELTA)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AccelerationCurve {
    Disabled,
    Linear { threshold: u32, maximum_milli: u16 },
}

/// Optional deterministic pointer acceleration curve.
///
/// Its representation is private so callers cannot bypass validated curve
/// construction by directly creating an invalid threshold or multiplier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerAcceleration(AccelerationCurve);

impl PointerAcceleration {
    /// Creates a curve that grows linearly from 1× at `threshold` to the given
    /// maximum at the input-domain bound.
    ///
    /// The same multiplier is applied to both axes, preserving the input
    /// direction and preventing axis-dependent distortion.
    ///
    /// # Errors
    ///
    /// Returns a configuration error when the threshold is not below the
    /// domain bound or the maximum is outside 1× through 4×.
    pub const fn linear(
        threshold: u32,
        maximum_milli: u16,
    ) -> Result<Self, NormalizationConfigError> {
        if threshold >= MAX_ABSOLUTE_FIXED_DELTA as u32 {
            Err(NormalizationConfigError::AccelerationThresholdOutOfRange)
        } else if maximum_milli < 1_000 || maximum_milli > MAX_ACCELERATION_MILLI {
            Err(NormalizationConfigError::AccelerationMultiplierOutOfRange)
        } else {
            Ok(Self(AccelerationCurve::Linear {
                threshold,
                maximum_milli,
            }))
        }
    }

    /// Returns a curve that applies no velocity-dependent gain.
    #[must_use]
    pub const fn disabled() -> Self {
        Self(AccelerationCurve::Disabled)
    }

    fn multiplier_milli(self, speed: u32) -> i64 {
        let AccelerationCurve::Linear {
            threshold,
            maximum_milli,
        } = self.0
        else {
            return GAIN_SCALE;
        };
        if speed <= threshold {
            return GAIN_SCALE;
        }
        let available = u64::from(MAX_ABSOLUTE_FIXED_DELTA as u32 - threshold);
        let distance = u64::from(speed - threshold);
        let extra = u64::from(maximum_milli - 1_000);
        let scaled = distance.saturating_mul(extra) / available;
        GAIN_SCALE + i64::try_from(scaled).unwrap_or(i64::MAX)
    }
}

impl Default for PointerAcceleration {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Complete deterministic pointer-normalization settings.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointerNormalizationConfig {
    sensitivity: Sensitivity,
    acceleration: PointerAcceleration,
    output_limit: NormalizationLimit,
}

impl PointerNormalizationConfig {
    #[must_use]
    pub const fn new(
        sensitivity: Sensitivity,
        acceleration: PointerAcceleration,
        output_limit: NormalizationLimit,
    ) -> Self {
        Self {
            sensitivity,
            acceleration,
            output_limit,
        }
    }

    #[must_use]
    pub const fn sensitivity(self) -> Sensitivity {
        self.sensitivity
    }

    #[must_use]
    pub const fn acceleration(self) -> PointerAcceleration {
        self.acceleration
    }

    #[must_use]
    pub const fn output_limit(self) -> NormalizationLimit {
        self.output_limit
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FractionalGain {
    remainder: i64,
}

impl FractionalGain {
    fn apply(
        &mut self,
        input: i32,
        sensitivity: Sensitivity,
        acceleration_milli: i64,
        limit: NormalizationLimit,
    ) -> i32 {
        let gain = i64::from(sensitivity.milli()) * acceleration_milli;
        let numerator = i64::from(input) * gain + self.remainder;
        let unbounded = numerator / COMBINED_GAIN_SCALE;
        let bounded = unbounded.clamp(-i64::from(limit.get()), i64::from(limit.get()));
        self.remainder = if bounded == unbounded {
            numerator % COMBINED_GAIN_SCALE
        } else {
            0
        };
        i32::try_from(bounded).unwrap_or_else(|_| unreachable!("validated output limit fits i32"))
    }

    fn reset(&mut self) {
        self.remainder = 0;
    }
}

/// Stateful pointer transformer with independent fractional axis remainders.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerNormalizer {
    config: PointerNormalizationConfig,
    x: FractionalGain,
    y: FractionalGain,
}

impl PointerNormalizer {
    #[must_use]
    pub const fn new(config: PointerNormalizationConfig) -> Self {
        Self {
            config,
            x: FractionalGain { remainder: 0 },
            y: FractionalGain { remainder: 0 },
        }
    }

    /// Applies sensitivity, optional acceleration, clamping, and fractional
    /// accumulation without converting to platform units.
    #[must_use]
    pub fn normalize(&mut self, input: PointerDelta) -> PointerDelta {
        let speed = input.x().unsigned_abs().max(input.y().unsigned_abs());
        let acceleration = self.config.acceleration.multiplier_milli(speed);
        let x = self.x.apply(
            input.x(),
            self.config.sensitivity,
            acceleration,
            self.config.output_limit,
        );
        let y = self.y.apply(
            input.y(),
            self.config.sensitivity,
            acceleration,
            self.config.output_limit,
        );
        PointerDelta::new(x, y).unwrap_or_else(|_| unreachable!("normalized output is clamped"))
    }

    /// Clears fractional history at a session, ownership, or lifecycle barrier.
    pub fn reset(&mut self) {
        self.x.reset();
        self.y.reset();
    }

    #[must_use]
    pub const fn config(&self) -> PointerNormalizationConfig {
        self.config
    }
}

impl Default for PointerNormalizer {
    fn default() -> Self {
        Self::new(PointerNormalizationConfig::default())
    }
}

/// Mapping from finger movement to scroll movement for one axis.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScrollDirection {
    /// Content follows finger movement.
    #[default]
    Natural,
    /// Content moves opposite finger movement.
    Traditional,
}

impl ScrollDirection {
    const fn apply(self, amount: i32) -> i32 {
        match self {
            Self::Natural => amount,
            Self::Traditional => -amount,
        }
    }
}

/// Settings for exactly one dedicated scroll axis.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScrollAxisConfig {
    sensitivity: Sensitivity,
    direction: ScrollDirection,
}

impl ScrollAxisConfig {
    #[must_use]
    pub const fn new(sensitivity: Sensitivity, direction: ScrollDirection) -> Self {
        Self {
            sensitivity,
            direction,
        }
    }

    #[must_use]
    pub const fn sensitivity(self) -> Sensitivity {
        self.sensitivity
    }

    #[must_use]
    pub const fn direction(self) -> ScrollDirection {
        self.direction
    }
}

/// Independent vertical and horizontal scroll-normalization settings.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScrollNormalizationConfig {
    vertical: ScrollAxisConfig,
    horizontal: ScrollAxisConfig,
    output_limit: NormalizationLimit,
}

impl ScrollNormalizationConfig {
    #[must_use]
    pub const fn new(
        vertical: ScrollAxisConfig,
        horizontal: ScrollAxisConfig,
        output_limit: NormalizationLimit,
    ) -> Self {
        Self {
            vertical,
            horizontal,
            output_limit,
        }
    }

    #[must_use]
    pub const fn axis(self, axis: ScrollAxis) -> ScrollAxisConfig {
        match axis {
            ScrollAxis::Horizontal => self.horizontal,
            ScrollAxis::Vertical => self.vertical,
        }
    }

    #[must_use]
    pub const fn output_limit(self) -> NormalizationLimit {
        self.output_limit
    }
}

/// Stateful scroll transformer whose axes never share fractional state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScrollNormalizer {
    config: ScrollNormalizationConfig,
    horizontal: FractionalGain,
    vertical: FractionalGain,
}

impl ScrollNormalizer {
    #[must_use]
    pub const fn new(config: ScrollNormalizationConfig) -> Self {
        Self {
            config,
            horizontal: FractionalGain { remainder: 0 },
            vertical: FractionalGain { remainder: 0 },
        }
    }

    /// Applies only the selected axis configuration and retains its remainder.
    #[must_use]
    pub fn normalize(&mut self, input: ScrollDelta) -> ScrollDelta {
        let axis_config = self.config.axis(input.axis());
        let directed = axis_config.direction.apply(input.amount());
        let accumulator = match input.axis() {
            ScrollAxis::Horizontal => &mut self.horizontal,
            ScrollAxis::Vertical => &mut self.vertical,
        };
        let amount = accumulator.apply(
            directed,
            axis_config.sensitivity,
            GAIN_SCALE,
            self.config.output_limit,
        );
        ScrollDelta::new(input.axis(), amount)
            .unwrap_or_else(|_| unreachable!("normalized scroll output is clamped"))
    }

    /// Clears both independent fractional histories.
    pub fn reset(&mut self) {
        self.horizontal.reset();
        self.vertical.reset();
    }
}

impl Default for ScrollNormalizer {
    fn default() -> Self {
        Self::new(ScrollNormalizationConfig::default())
    }
}

/// Positive number of normalized fixed-point units per one native unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeUnitQuantum(u32);

impl NativeUnitQuantum {
    /// Creates a platform-selected native-unit quantum.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizationConfigError::NativeQuantumIsZero`] for zero.
    pub const fn new(fixed_units: u32) -> Result<Self, NormalizationConfigError> {
        if fixed_units == 0 {
            Err(NormalizationConfigError::NativeQuantumIsZero)
        } else {
            Ok(Self(fixed_units))
        }
    }

    #[must_use]
    pub const fn fixed_units(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeAxisAccumulator {
    quantum: NativeUnitQuantum,
    remainder: i64,
}

impl NativeAxisAccumulator {
    const fn new(quantum: NativeUnitQuantum) -> Self {
        Self {
            quantum,
            remainder: 0,
        }
    }

    fn translate(&mut self, fixed_units: i32) -> i32 {
        let total = i64::from(fixed_units) + self.remainder;
        let quantum = i64::from(self.quantum.fixed_units());
        let native_units = total / quantum;
        self.remainder = total % quantum;
        i32::try_from(native_units)
            .unwrap_or_else(|_| unreachable!("bounded normalized delta fits native units"))
    }

    fn reset(&mut self) {
        self.remainder = 0;
    }
}

/// Integral platform pointer movement produced by a translation hook.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct NativePointerDelta {
    x: i32,
    y: i32,
}

impl NativePointerDelta {
    #[must_use]
    pub const fn x(self) -> i32 {
        self.x
    }

    #[must_use]
    pub const fn y(self) -> i32 {
        self.y
    }
}

impl fmt::Debug for NativePointerDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NativePointerDelta(<redacted>)")
    }
}

/// Per-axis fractional hook for platform pointer-unit conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerUnitTranslator {
    x: NativeAxisAccumulator,
    y: NativeAxisAccumulator,
}

impl PointerUnitTranslator {
    #[must_use]
    pub const fn new(x: NativeUnitQuantum, y: NativeUnitQuantum) -> Self {
        Self {
            x: NativeAxisAccumulator::new(x),
            y: NativeAxisAccumulator::new(y),
        }
    }

    #[must_use]
    pub fn translate(&mut self, input: PointerDelta) -> NativePointerDelta {
        NativePointerDelta {
            x: self.x.translate(input.x()),
            y: self.y.translate(input.y()),
        }
    }

    pub fn reset(&mut self) {
        self.x.reset();
        self.y.reset();
    }
}

/// Integral single-axis native scroll output, including zero while accumulating.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DiscreteScrollDelta {
    axis: ScrollAxis,
    steps: i32,
}

impl fmt::Debug for DiscreteScrollDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DiscreteScrollDelta(<redacted>)")
    }
}

impl DiscreteScrollDelta {
    #[must_use]
    pub const fn axis(self) -> ScrollAxis {
        self.axis
    }

    #[must_use]
    pub const fn steps(self) -> i32 {
        self.steps
    }
}

/// Independent fractional conversion hook for discrete native scroll backends.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiscreteScrollTranslator {
    horizontal: NativeAxisAccumulator,
    vertical: NativeAxisAccumulator,
}

impl DiscreteScrollTranslator {
    #[must_use]
    pub const fn new(horizontal: NativeUnitQuantum, vertical: NativeUnitQuantum) -> Self {
        Self {
            horizontal: NativeAxisAccumulator::new(horizontal),
            vertical: NativeAxisAccumulator::new(vertical),
        }
    }

    #[must_use]
    pub fn translate(&mut self, input: ScrollDelta) -> DiscreteScrollDelta {
        let accumulator = match input.axis() {
            ScrollAxis::Horizontal => &mut self.horizontal,
            ScrollAxis::Vertical => &mut self.vertical,
        };
        DiscreteScrollDelta {
            axis: input.axis(),
            steps: accumulator.translate(input.amount()),
        }
    }

    pub fn reset(&mut self) {
        self.horizontal.reset();
        self.vertical.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tether_test_support::SeededRandom;

    fn pointer(x: i32, y: i32) -> PointerDelta {
        PointerDelta::new(x, y).expect("test pointer is in range")
    }

    fn scroll(axis: ScrollAxis, amount: i32) -> ScrollDelta {
        ScrollDelta::new(axis, amount).expect("test scroll is in range")
    }

    fn sample_delta(random: &mut SeededRandom) -> i32 {
        let span = u64::from(MAX_ABSOLUTE_FIXED_DELTA as u32) * 2 + 1;
        let centered = i64::try_from(random.next_u64() % span).expect("sample fits i64")
            - i64::from(MAX_ABSOLUTE_FIXED_DELTA);
        i32::try_from(centered).expect("sample is in the input domain")
    }

    #[test]
    fn configuration_rejects_unsafe_ranges() {
        assert_eq!(
            Sensitivity::from_milli(MIN_SENSITIVITY_MILLI - 1),
            Err(NormalizationConfigError::SensitivityOutOfRange)
        );
        assert_eq!(
            Sensitivity::from_milli(MAX_SENSITIVITY_MILLI + 1),
            Err(NormalizationConfigError::SensitivityOutOfRange)
        );
        assert_eq!(
            PointerAcceleration::linear(MAX_ABSOLUTE_FIXED_DELTA as u32, 2_000),
            Err(NormalizationConfigError::AccelerationThresholdOutOfRange)
        );
        assert_eq!(
            PointerAcceleration::linear(0, MAX_ACCELERATION_MILLI + 1),
            Err(NormalizationConfigError::AccelerationMultiplierOutOfRange)
        );
        assert_eq!(
            NormalizationLimit::new(0),
            Err(NormalizationConfigError::OutputLimitOutOfRange)
        );
        assert_eq!(
            NativeUnitQuantum::new(0),
            Err(NormalizationConfigError::NativeQuantumIsZero)
        );
    }

    #[test]
    fn pointer_fractional_gain_accumulates_and_resets() {
        let config = PointerNormalizationConfig::new(
            Sensitivity::from_milli(125).expect("valid sensitivity"),
            PointerAcceleration::disabled(),
            NormalizationLimit::default(),
        );
        let mut normalizer = PointerNormalizer::new(config);
        for _ in 0..7 {
            assert_eq!(normalizer.normalize(pointer(1, 0)), pointer(0, 0));
        }
        assert_eq!(normalizer.normalize(pointer(1, 0)), pointer(1, 0));
        let _ = normalizer.normalize(pointer(7, 0));
        normalizer.reset();
        assert_eq!(normalizer.normalize(pointer(1, 0)), pointer(0, 0));
    }

    #[test]
    fn acceleration_is_monotonic_and_uses_one_multiplier_for_both_axes() {
        let acceleration = PointerAcceleration::linear(100, 3_000).expect("valid curve");
        assert_eq!(acceleration.multiplier_milli(100), 1_000);
        assert!(acceleration.multiplier_milli(10_000) > 1_000);
        assert_eq!(
            acceleration.multiplier_milli(MAX_ABSOLUTE_FIXED_DELTA as u32),
            3_000
        );
        let config = PointerNormalizationConfig::new(
            Sensitivity::unity(),
            acceleration,
            NormalizationLimit::default(),
        );
        let mut normalizer = PointerNormalizer::new(config);
        let output = normalizer.normalize(pointer(20_000, 10_000));
        assert_eq!(output.x(), output.y() * 2);
    }

    #[test]
    fn scroll_axes_have_independent_direction_sensitivity_and_remainders() {
        let config = ScrollNormalizationConfig::new(
            ScrollAxisConfig::new(
                Sensitivity::from_milli(500).expect("valid"),
                ScrollDirection::Natural,
            ),
            ScrollAxisConfig::new(Sensitivity::unity(), ScrollDirection::Traditional),
            NormalizationLimit::default(),
        );
        let mut normalizer = ScrollNormalizer::new(config);
        assert_eq!(
            normalizer.normalize(scroll(ScrollAxis::Vertical, 1)),
            scroll(ScrollAxis::Vertical, 0)
        );
        assert_eq!(
            normalizer.normalize(scroll(ScrollAxis::Horizontal, 1)),
            scroll(ScrollAxis::Horizontal, -1)
        );
        assert_eq!(
            normalizer.normalize(scroll(ScrollAxis::Vertical, 1)),
            scroll(ScrollAxis::Vertical, 1)
        );
    }

    #[test]
    fn normalization_clamps_without_leaking_overload_into_later_samples() {
        let config = PointerNormalizationConfig::new(
            Sensitivity::from_milli(8_000).expect("valid"),
            PointerAcceleration::linear(0, 4_000).expect("valid"),
            NormalizationLimit::new(100).expect("valid"),
        );
        let mut normalizer = PointerNormalizer::new(config);
        assert_eq!(
            normalizer.normalize(pointer(MAX_ABSOLUTE_FIXED_DELTA, 0)),
            pointer(100, 0)
        );
        assert_eq!(normalizer.normalize(pointer(0, 0)), pointer(0, 0));
    }

    #[test]
    fn native_translation_accumulates_symmetrically_and_keeps_axes_separate() {
        let quantum = NativeUnitQuantum::new(1_024).expect("valid quantum");
        let mut pointer_translator = PointerUnitTranslator::new(quantum, quantum);
        assert_eq!(
            pointer_translator.translate(pointer(512, 0)),
            NativePointerDelta { x: 0, y: 0 }
        );
        assert_eq!(
            pointer_translator.translate(pointer(512, -1_024)),
            NativePointerDelta { x: 1, y: -1 }
        );

        let mut scroll_translator = DiscreteScrollTranslator::new(quantum, quantum);
        assert_eq!(
            scroll_translator
                .translate(scroll(ScrollAxis::Vertical, 512))
                .steps(),
            0
        );
        assert_eq!(
            scroll_translator
                .translate(scroll(ScrollAxis::Horizontal, 1_024))
                .steps(),
            1
        );
        assert_eq!(
            scroll_translator
                .translate(scroll(ScrollAxis::Vertical, -512))
                .steps(),
            0
        );
    }

    #[test]
    fn seeded_pointer_properties_preserve_bounds_direction_and_axis_isolation() {
        let config = PointerNormalizationConfig::new(
            Sensitivity::from_milli(8_000).expect("valid"),
            PointerAcceleration::linear(250, 4_000).expect("valid"),
            NormalizationLimit::new(65_536).expect("valid"),
        );
        let mut random = SeededRandom::new(0x5445_5448_4552);
        for _ in 0..20_000 {
            let x = sample_delta(&mut random);
            let y = sample_delta(&mut random);
            let mut normalizer = PointerNormalizer::new(config);
            let output = normalizer.normalize(pointer(x, y));
            assert!(output.x().unsigned_abs() <= 65_536);
            assert!(output.y().unsigned_abs() <= 65_536);
            assert!(output.x() == 0 || output.x().signum() == x.signum());
            assert!(output.y() == 0 || output.y().signum() == y.signum());

            let mut x_isolation = PointerNormalizer::new(config);
            let isolated = x_isolation.normalize(pointer(0, y));
            assert_eq!(isolated.x(), 0);
        }
    }

    #[test]
    fn seeded_scroll_properties_never_change_or_leak_axis() {
        let config = ScrollNormalizationConfig::new(
            ScrollAxisConfig::new(
                Sensitivity::from_milli(7_500).expect("valid"),
                ScrollDirection::Natural,
            ),
            ScrollAxisConfig::new(
                Sensitivity::from_milli(125).expect("valid"),
                ScrollDirection::Traditional,
            ),
            NormalizationLimit::new(32_768).expect("valid"),
        );
        let mut random = SeededRandom::new(0x5343_524f_4c4c);
        let mut normalizer = ScrollNormalizer::new(config);
        for index in 0..20_000 {
            let axis = if index % 2 == 0 {
                ScrollAxis::Vertical
            } else {
                ScrollAxis::Horizontal
            };
            let input = sample_delta(&mut random);
            let output = normalizer.normalize(scroll(axis, input));
            assert_eq!(output.axis(), axis);
            assert!(output.amount().unsigned_abs() <= 32_768);
            let expected_sign = match axis {
                ScrollAxis::Vertical => input.signum(),
                ScrollAxis::Horizontal => -input.signum(),
            };
            assert!(output.amount() == 0 || output.amount().signum() == expected_sign);
        }
    }
}
