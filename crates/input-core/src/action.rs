use std::fmt;

/// Number of fixed-point units in one normalized input unit.
pub const FIXED_POINT_SCALE: i32 = 1_024;

/// Hard domain bound for either component of a normalized delta.
///
/// This is intentionally lower than `i32::MAX` so later fixed-point
/// multiplication can use checked arithmetic with useful headroom.
pub const MAX_ABSOLUTE_FIXED_DELTA: i32 = 8_388_608;

/// Maximum encoded UTF-8 bytes accepted by one text insertion action.
pub const MAX_TEXT_BYTES: usize = 4_096;

/// Safe validation failures for platform-neutral values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueError {
    /// A fixed-point component exceeded the domain hard limit.
    DeltaOutOfRange,
    /// A physical key was outside the supported USB HID usage page.
    InvalidPhysicalKey,
    /// Atomic click count was neither one nor two.
    InvalidClickCount,
    /// Text was empty.
    EmptyText,
    /// Text exceeded [`MAX_TEXT_BYTES`].
    TextTooLong,
}

impl fmt::Display for ValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::DeltaOutOfRange => "input delta is outside the allowed range",
            Self::InvalidPhysicalKey => "physical key is outside the allowlist",
            Self::InvalidClickCount => "atomic click count must be one or two",
            Self::EmptyText => "text action is empty",
            Self::TextTooLong => "text action exceeds the byte limit",
        })
    }
}

impl std::error::Error for ValueError {}

fn validate_delta(value: i32) -> Result<i32, ValueError> {
    if value.unsigned_abs() > MAX_ABSOLUTE_FIXED_DELTA as u32 {
        Err(ValueError::DeltaOutOfRange)
    } else {
        Ok(value)
    }
}

/// Relative two-axis pointer motion in 1/1024 normalized units.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PointerDelta {
    x: i32,
    y: i32,
}

impl PointerDelta {
    /// Validates and constructs a pointer delta.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::DeltaOutOfRange`] if either component exceeds the
    /// hard domain limit.
    pub fn new(x: i32, y: i32) -> Result<Self, ValueError> {
        Ok(Self {
            x: validate_delta(x)?,
            y: validate_delta(y)?,
        })
    }

    /// Returns the horizontal component.
    #[must_use]
    pub const fn x(self) -> i32 {
        self.x
    }

    /// Returns the vertical component.
    #[must_use]
    pub const fn y(self) -> i32 {
        self.y
    }
}

impl fmt::Debug for PointerDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PointerDelta(<redacted>)")
    }
}

/// Scroll axis selected by a dedicated scroll strip.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScrollAxis {
    /// Horizontal scrolling from the bottom strip.
    Horizontal,
    /// Vertical scrolling from the configured side strip.
    Vertical,
}

/// Single-axis scroll motion in 1/1024 normalized units.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ScrollDelta {
    axis: ScrollAxis,
    amount: i32,
}

impl ScrollDelta {
    /// Validates and constructs a single-axis scroll delta.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::DeltaOutOfRange`] when the amount exceeds the hard
    /// domain limit.
    pub fn new(axis: ScrollAxis, amount: i32) -> Result<Self, ValueError> {
        Ok(Self {
            axis,
            amount: validate_delta(amount)?,
        })
    }

    /// Returns the selected axis.
    #[must_use]
    pub const fn axis(self) -> ScrollAxis {
        self.axis
    }

    /// Returns the signed fixed-point amount.
    #[must_use]
    pub const fn amount(self) -> i32 {
        self.amount
    }
}

impl fmt::Debug for ScrollDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ScrollDelta(<redacted>)")
    }
}

/// Closed set of mouse buttons exposed by the product.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl fmt::Debug for MouseButton {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MouseButton(<redacted>)")
    }
}

/// Mouse-button transition or neutral atomic click.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ButtonAction {
    Down(MouseButton),
    Up(MouseButton),
    Click {
        button: MouseButton,
        count: ClickCount,
    },
}

/// Validated atomic click count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClickCount {
    Single,
    Double,
}

impl TryFrom<u8> for ClickCount {
    type Error = ValueError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Single),
            2 => Ok(Self::Double),
            _ => Err(ValueError::InvalidClickCount),
        }
    }
}

impl ButtonAction {
    /// Creates an atomic single or double click.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::InvalidClickCount`] when `count` is not one or two.
    pub fn click(button: MouseButton, count: u8) -> Result<Self, ValueError> {
        Ok(Self::Click {
            button,
            count: count.try_into()?,
        })
    }
}

impl fmt::Debug for ButtonAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ButtonAction(<redacted>)")
    }
}

/// Allowlisted USB HID Keyboard/Keypad-page usage.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalKey(u8);

impl PhysicalKey {
    /// USB HID usage page for all values represented by this type.
    pub const USAGE_PAGE: u16 = 0x07;

    /// Constructs a physical key from a Keyboard/Keypad usage identifier.
    ///
    /// The allowlist includes standard keyboard usages, F13–F24, and modifiers,
    /// while excluding reserved and power usages.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::InvalidPhysicalKey`] for an unknown or intentionally
    /// unsupported usage.
    pub fn new(usage: u16) -> Result<Self, ValueError> {
        let usage = u8::try_from(usage).map_err(|_| ValueError::InvalidPhysicalKey)?;
        if matches!(usage, 0x04..=0x65 | 0x67..=0xA4 | 0xB0..=0xDD | 0xE0..=0xE7) {
            Ok(Self(usage))
        } else {
            Err(ValueError::InvalidPhysicalKey)
        }
    }

    /// Returns the USB HID usage for native translation only.
    #[must_use]
    pub const fn usage(self) -> u8 {
        self.0
    }

    /// Returns whether this usage is a keyboard modifier.
    #[must_use]
    pub const fn is_modifier(self) -> bool {
        self.0 >= 0xE0 && self.0 <= 0xE7
    }
}

impl fmt::Debug for PhysicalKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PhysicalKey(<redacted>)")
    }
}

/// Physical-key transition.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum KeyAction {
    Down(PhysicalKey),
    Up(PhysicalKey),
}

impl fmt::Debug for KeyAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KeyAction(<redacted>)")
    }
}

/// Bounded UTF-8 text whose standard formatting is always redacted.
#[derive(Clone, Eq, PartialEq)]
pub struct SensitiveText(String);

impl SensitiveText {
    /// Validates and owns text prepared for explicit insertion.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::EmptyText`] for an empty value or
    /// [`ValueError::TextTooLong`] above [`MAX_TEXT_BYTES`].
    pub fn new(value: String) -> Result<Self, ValueError> {
        if value.is_empty() {
            Err(ValueError::EmptyText)
        } else if value.len() > MAX_TEXT_BYTES {
            Err(ValueError::TextTooLong)
        } else {
            Ok(Self(value))
        }
    }

    /// Exposes text only at the final native-injection boundary.
    ///
    /// Callers must not log, persist, trace, or include this value in an error.
    #[must_use]
    pub fn expose_for_injection(&self) -> &str {
        &self.0
    }

    /// Returns the encoded size without exposing content.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for SensitiveText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SensitiveText(<redacted>)")
    }
}

impl fmt::Display for SensitiveText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted text>")
    }
}

/// Layout-independent editing command.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TextEditCommand {
    DeleteBackward,
    DeleteForward,
    InsertLineBreak,
}

/// Reviewed application command rather than an arbitrary key chord.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LogicalCommand {
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    SelectAll,
    Find,
    Save,
    CloseTab,
    SwitchApplication,
    TaskManagerEquivalent,
}

/// Closed media command set.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MediaCommand {
    PlayPause,
    NextTrack,
    PreviousTrack,
    VolumeUp,
    VolumeDown,
    Mute,
}

/// Semantic operation class safe to include in diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationClass {
    PointerMove,
    Scroll,
    MouseButton,
    PhysicalKey,
    TextInsert,
    TextEdit,
    LogicalCommand,
    MediaCommand,
}

/// Platform-neutral input action accepted by the serialized dispatcher.
#[derive(Clone, Eq, PartialEq)]
pub enum InputAction {
    PointerMove(PointerDelta),
    Scroll(ScrollDelta),
    MouseButton(ButtonAction),
    PhysicalKey(KeyAction),
    TextInsert(SensitiveText),
    TextEdit(TextEditCommand),
    LogicalCommand(LogicalCommand),
    MediaCommand(MediaCommand),
}

impl InputAction {
    /// Returns the safe operation class without inspecting the payload.
    #[must_use]
    pub const fn class(&self) -> OperationClass {
        match self {
            Self::PointerMove(_) => OperationClass::PointerMove,
            Self::Scroll(_) => OperationClass::Scroll,
            Self::MouseButton(_) => OperationClass::MouseButton,
            Self::PhysicalKey(_) => OperationClass::PhysicalKey,
            Self::TextInsert(_) => OperationClass::TextInsert,
            Self::TextEdit(_) => OperationClass::TextEdit,
            Self::LogicalCommand(_) => OperationClass::LogicalCommand,
            Self::MediaCommand(_) => OperationClass::MediaCommand,
        }
    }
}

impl fmt::Debug for InputAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InputAction")
            .field("class", &self.class())
            .field("payload", &"<redacted>")
            .finish()
    }
}
