# Input normalization and native-unit translation

- **Task:** IC-004
- **Status:** Implemented contract
- **Authority:** `SPEC.md` sections 8–11, 21, 24, 28, and 33

This component converts already validated fixed-point pointer and scroll deltas
into bounded normalized deltas, then provides stateful hooks that native
backends can use to convert normalized fixed-point units into integral platform
units. It is platform-neutral and performs no input injection.

## Pipeline position

Normalization occurs after protocol/domain validation and before submission to
the ordered dispatcher. Native-unit translation occurs inside the selected
backend on the dispatcher's exclusive execution context. Neither stage may
reorder actions or move deltas across a click, key/button transition, ownership
operation, or release barrier.

The browser gesture engine remains responsible for turning CSS-pixel samples
into normalized protocol deltas and for momentum. This component does not
duplicate gesture classification or kinetic-scroll timing.

## Numeric model

- Input and normalized output use the domain's signed 1/1024 fixed-point units.
- Sensitivity is validated integer thousandths from 0.125× through 8×; 1× is
  the default. Floating-point NaN, infinity, and platform-dependent rounding
  cannot enter the calculation.
- Pointer acceleration is either disabled or a validated linear curve. The
  curve is 1× at and below its threshold and increases to a configured maximum
  of at most 4× at the domain bound.
- Speed uses the maximum absolute input component. One acceleration multiplier
  is applied to both pointer axes, which preserves direction and prevents
  axis-dependent distortion.
- Every output component is clamped to a validated positive bound no larger
  than the domain hard limit. A clamped sample clears that axis's discarded
  fractional overload so it cannot appear in a later movement.
- Intermediate arithmetic is widened to `i64`. Configuration representations
  are private or validated types, so safe callers cannot construct arithmetic
  outside the proven ranges.

No smoothing is enabled in this baseline because the specification requires it
only where platform/device evidence shows a need. Adding history-dependent
smoothing requires measured evidence, explicit reset behavior, and property
tests; it must not be hidden inside a backend.

## Fractional state and reset

Pointer X/Y and scroll horizontal/vertical maintain independent signed
remainders. Truncation is symmetric toward zero, so equal opposite sub-unit
motion cancels without directional bias. Scroll direction and sensitivity are
configured independently per axis; horizontal state can never contribute to a
vertical result or vice versa.

Call `reset` on all normalizers and translators at session epoch changes,
ownership changes, disconnect/release, suspend/lock, backend replacement, and
configuration replacement. Fractional history is never restored after a
reconnect.

## Platform translation hooks

`PointerUnitTranslator` accepts an independent positive normalized-unit quantum
for each pointer axis and yields integral native X/Y movement while retaining
sub-unit remainders. `DiscreteScrollTranslator` does the same independently for
horizontal and vertical scroll. A zero result means more normalized movement is
needed; it is not a failure.

X11 uses the discrete scroll result to emit one balanced Button 4/5 or 6/7
press/release pair per signed step. The translator does not emit those events
itself and does not select platform conventions. Continuous-scroll backends may
use their native precision and retain any backend-specific remainder through an
equivalent reviewed adapter.

## Safety and diagnostics

Raw and native pointer/scroll delta values have redacted standard `Debug`
representations. Configuration errors are closed non-sensitive categories.
No stage logs input values. All state is bounded and allocation-free after
construction.

## Automated coverage

Unit and deterministic seeded property tests cover invalid configuration,
fraction retention and reset, acceleration endpoints and monotonicity,
clamping, sign/direction preservation, hard output bounds, pointer axis
isolation, scroll axis/configuration isolation, symmetric cancellation, and
independent platform-unit accumulators. The property loops exercise 20,000
pointer pairs and 20,000 alternating-axis scroll values over the entire domain.

These tests execute no native input and do not validate subjective pointer feel,
operating-system acceleration, DPI behavior, multiple monitors, wheel
conventions, or native applications. Those remain explicit platform hardware
gates.
