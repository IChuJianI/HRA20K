//! Key codes, logical keys and modifier state.

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use smol_str::SmolStr;

use crate::event::ElementState;

/// The location of a key on the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyLocation {
    #[default]
    Standard,
    Left,
    Right,
    Numpad,
}

/// A physical key code, independent of layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[allow(non_camel_case_types)]
pub enum KeyCode {
    Backquote, Backslash, BracketLeft, BracketRight, Comma,
    Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    Equal, IntlBackslash, IntlRo, IntlYen,
    KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM,
    KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    Minus, Period, Quote, Semicolon, Slash,
    AltLeft, AltRight, Backspace, CapsLock, ContextMenu, ControlLeft, ControlRight,
    Enter, SuperLeft, SuperRight, ShiftLeft, ShiftRight, Space, Tab,
    Convert, KanaMode, Lang1, Lang2, Lang3, Lang4, Lang5, NonConvert,
    Delete, End, Help, Home, Insert, PageDown, PageUp,
    ArrowDown, ArrowLeft, ArrowRight, ArrowUp,
    NumLock,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7,
    Numpad8, Numpad9, NumpadAdd, NumpadBackspace, NumpadClear, NumpadClearEntry,
    NumpadComma, NumpadDecimal, NumpadDivide, NumpadEnter, NumpadEqual, NumpadHash,
    NumpadMemoryAdd, NumpadMemoryClear, NumpadMemoryRecall, NumpadMemoryStore,
    NumpadMemorySubtract, NumpadMultiply, NumpadParenLeft, NumpadParenRight,
    NumpadStar, NumpadSubtract,
    Escape, Fn, FnLock, PrintScreen, ScrollLock, Pause,
    BrowserBack, BrowserFavorites, BrowserForward, BrowserHome, BrowserRefresh,
    BrowserSearch, BrowserStop, Eject, LaunchApp1, LaunchApp2, LaunchMail,
    MediaPlayPause, MediaSelect, MediaStop, MediaTrackNext, MediaTrackPrevious,
    Power, Sleep, AudioVolumeDown, AudioVolumeMute, AudioVolumeUp, WakeUp,
    Meta, Hyper, Turbo, Abort, Resume, Suspend, Again, Copy, Cut, Find, Open,
    Paste, Props, Select, Undo, Hiragana, Katakana,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
    F25, F26, F27, F28, F29, F30, F31, F32, F33, F34, F35,
}

/// A layout-dependent named key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum NamedKey {
    Alt, AltGraph, CapsLock, Control, Fn, FnLock, NumLock, ScrollLock, Shift,
    Symbol, SymbolLock, Meta, Hyper, Super,
    Enter, Tab, Space, ArrowDown, ArrowLeft, ArrowRight, ArrowUp,
    End, Home, PageDown, PageUp, Backspace, Clear, Copy, Cut, Delete, Insert,
    Paste, Redo, Undo, Accept, Again, Cancel, ContextMenu, Escape, Execute,
    Find, Help, Pause, Play, Props, Select, ZoomIn, ZoomOut,
    Eject, Power, PowerOff, PrintScreen, Hibernate, Standby, WakeUp,
    Convert, NonConvert, KanaMode, Katakana, Hiragana, Hankaku,
    Open, Save, Print,
    AudioVolumeDown, AudioVolumeUp, AudioVolumeMute,
    LaunchApplication1, LaunchApplication2, LaunchMail,
    BrowserBack, BrowserFavorites, BrowserForward, BrowserHome, BrowserRefresh,
    BrowserSearch, BrowserStop,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
    F25, F26, F27, F28, F29, F30, F31, F32, F33, F34, F35,
}

/// A platform-specific key that has no cross-platform name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeKey {
    Unidentified(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeKeyCode {
    Unidentified(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysicalKey {
    Code(KeyCode),
    Unidentified(NativeKeyCode),
}

/// A logical key, with or without the layout's character.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key<Str = SmolStr> {
    Named(NamedKey),
    Character(Str),
    Unidentified(NativeKey),
    Dead(Option<char>),
}

impl Key {
    pub fn to_text(&self) -> Option<&str> {
        match self {
            Self::Character(text) => Some(text.as_str()),
            _ => None,
        }
    }
}

/// The state of the eight keyboard modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ModifiersState(u8);

impl ModifiersState {
    pub const SHIFT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);
    pub const ALT_GRAPH: Self = Self(1 << 4);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(0b1_1111)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & 0b1_1111)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn set(&mut self, other: Self, value: bool) {
        if value {
            self.0 |= other.0;
        } else {
            self.0 &= !other.0;
        }
    }

    pub const fn shift_key(self) -> bool {
        self.contains(Self::SHIFT)
    }

    pub const fn control_key(self) -> bool {
        self.contains(Self::CONTROL)
    }

    pub const fn alt_key(self) -> bool {
        self.contains(Self::ALT)
    }

    pub const fn super_key(self) -> bool {
        self.contains(Self::SUPER)
    }
}

impl BitOr for ModifiersState {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ModifiersState {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ModifiersState {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for ModifiersState {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for ModifiersState {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0 & 0b1_1111)
    }
}

/// The modifier state reported by `WindowEvent::ModifiersChanged`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    state: ModifiersState,
}

impl Modifiers {
    pub const fn new(state: ModifiersState) -> Self {
        Self { state }
    }

    pub const fn state(&self) -> ModifiersState {
        self.state
    }

    pub const fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    pub const fn contains(&self, state: ModifiersState) -> bool {
        self.state.contains(state)
    }

    pub const fn shift_key(&self) -> bool {
        self.state.shift_key()
    }

    pub const fn control_key(&self) -> bool {
        self.state.control_key()
    }

    pub const fn alt_key(&self) -> bool {
        self.state.alt_key()
    }

    pub const fn super_key(&self) -> bool {
        self.state.super_key()
    }
}

impl From<ModifiersState> for Modifiers {
    fn from(state: ModifiersState) -> Self {
        Self { state }
    }
}

/// Extra data attached to a key event by the platform.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct KeyEventExtra {
    pub text_with_all_modifiers: Option<SmolStr>,
    pub key_without_modifiers: Option<Box<Key>>,
}

/// A keyboard input event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub physical_key: PhysicalKey,
    pub logical_key: Key,
    pub text: Option<SmolStr>,
    pub location: KeyLocation,
    pub state: ElementState,
    pub repeat: bool,
    pub platform_specific: KeyEventExtra,
}

/// Platform-specific key event extensions used by the engine.
pub trait KeyEventExtModifierSupplement {
    fn text_with_all_modifiers(&self) -> Option<&str>;
    fn key_without_modifiers(&self) -> Key;
}

impl KeyEventExtModifierSupplement for KeyEvent {
    fn text_with_all_modifiers(&self) -> Option<&str> {
        self.platform_specific
            .text_with_all_modifiers
            .as_deref()
            .or(self.text.as_deref())
    }

    fn key_without_modifiers(&self) -> Key {
        self.platform_specific
            .key_without_modifiers
            .as_deref()
            .cloned()
            .unwrap_or_else(|| self.logical_key.clone())
    }
}
