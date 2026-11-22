//! Physical and logical pixel sizes and positions.

/// A numeric pixel type.
pub trait Pixel: Copy + Into<f64> {
    fn from_f64(value: f64) -> Self;
}

impl Pixel for f64 {
    fn from_f64(value: f64) -> Self {
        value
    }
}

impl Pixel for f32 {
    fn from_f64(value: f64) -> Self {
        value as f32
    }
}

impl Pixel for u32 {
    fn from_f64(value: f64) -> Self {
        value.max(0.0).round() as u32
    }
}

impl Pixel for i32 {
    fn from_f64(value: f64) -> Self {
        value.round() as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PhysicalSize<P = u32> {
    pub width: P,
    pub height: P,
}

impl<P> PhysicalSize<P> {
    pub const fn new(width: P, height: P) -> Self {
        Self { width, height }
    }
}

impl<P: Pixel> PhysicalSize<P> {
    pub fn cast<X: Pixel>(&self) -> PhysicalSize<X> {
        PhysicalSize::new(X::from_f64(self.width.into()), X::from_f64(self.height.into()))
    }

    pub fn to_logical<X: Pixel>(&self, scale_factor: f64) -> LogicalSize<X> {
        let scale = if scale_factor > 0.0 { scale_factor } else { 1.0 };
        LogicalSize::new(
            X::from_f64(self.width.into() / scale),
            X::from_f64(self.height.into() / scale),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LogicalSize<P = f64> {
    pub width: P,
    pub height: P,
}

impl<P> LogicalSize<P> {
    pub const fn new(width: P, height: P) -> Self {
        Self { width, height }
    }
}

impl<P: Pixel> LogicalSize<P> {
    pub fn cast<X: Pixel>(&self) -> LogicalSize<X> {
        LogicalSize::new(X::from_f64(self.width.into()), X::from_f64(self.height.into()))
    }

    pub fn to_physical<X: Pixel>(&self, scale_factor: f64) -> PhysicalSize<X> {
        let scale = if scale_factor > 0.0 { scale_factor } else { 1.0 };
        PhysicalSize::new(
            X::from_f64(self.width.into() * scale),
            X::from_f64(self.height.into() * scale),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PhysicalPosition<P = i32> {
    pub x: P,
    pub y: P,
}

impl<P> PhysicalPosition<P> {
    pub const fn new(x: P, y: P) -> Self {
        Self { x, y }
    }
}

impl<P: Pixel> PhysicalPosition<P> {
    pub fn cast<X: Pixel>(&self) -> PhysicalPosition<X> {
        PhysicalPosition::new(X::from_f64(self.x.into()), X::from_f64(self.y.into()))
    }

    pub fn to_logical<X: Pixel>(&self, scale_factor: f64) -> LogicalPosition<X> {
        let scale = if scale_factor > 0.0 { scale_factor } else { 1.0 };
        LogicalPosition::new(
            X::from_f64(self.x.into() / scale),
            X::from_f64(self.y.into() / scale),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LogicalPosition<P = f64> {
    pub x: P,
    pub y: P,
}

impl<P> LogicalPosition<P> {
    pub const fn new(x: P, y: P) -> Self {
        Self { x, y }
    }
}

impl<P: Pixel> LogicalPosition<P> {
    pub fn cast<X: Pixel>(&self) -> LogicalPosition<X> {
        LogicalPosition::new(X::from_f64(self.x.into()), X::from_f64(self.y.into()))
    }

    pub fn to_physical<X: Pixel>(&self, scale_factor: f64) -> PhysicalPosition<X> {
        let scale = if scale_factor > 0.0 { scale_factor } else { 1.0 };
        PhysicalPosition::new(
            X::from_f64(self.x.into() * scale),
            X::from_f64(self.y.into() * scale),
        )
    }
}

/// Either a physical or a logical size, as accepted by window attribute setters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    Physical(PhysicalSize<u32>),
    Logical(LogicalSize<f64>),
}

impl Size {
    pub fn to_physical(&self, scale_factor: f64) -> PhysicalSize<u32> {
        match self {
            Self::Physical(size) => *size,
            Self::Logical(size) => size.to_physical(scale_factor),
        }
    }

    pub fn to_logical(&self, scale_factor: f64) -> LogicalSize<f64> {
        match self {
            Self::Physical(size) => size.to_logical(scale_factor),
            Self::Logical(size) => *size,
        }
    }
}

impl From<PhysicalSize<u32>> for Size {
    fn from(size: PhysicalSize<u32>) -> Self {
        Self::Physical(size)
    }
}

impl From<LogicalSize<f64>> for Size {
    fn from(size: LogicalSize<f64>) -> Self {
        Self::Logical(size)
    }
}

impl From<LogicalSize<u32>> for Size {
    fn from(size: LogicalSize<u32>) -> Self {
        Self::Logical(LogicalSize::new(size.width.into(), size.height.into()))
    }
}

impl From<PhysicalSize<f64>> for Size {
    fn from(size: PhysicalSize<f64>) -> Self {
        Self::Physical(size.cast())
    }
}

/// Either a physical or a logical position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Position {
    Physical(PhysicalPosition<i32>),
    Logical(LogicalPosition<f64>),
}

impl Position {
    pub fn to_physical(&self, scale_factor: f64) -> PhysicalPosition<i32> {
        match self {
            Self::Physical(position) => *position,
            Self::Logical(position) => position.to_physical(scale_factor),
        }
    }

    pub fn to_logical(&self, scale_factor: f64) -> LogicalPosition<f64> {
        match self {
            Self::Physical(position) => position.to_logical(scale_factor),
            Self::Logical(position) => *position,
        }
    }
}

impl From<PhysicalPosition<i32>> for Position {
    fn from(position: PhysicalPosition<i32>) -> Self {
        Self::Physical(position)
    }
}

impl From<LogicalPosition<f64>> for Position {
    fn from(position: LogicalPosition<f64>) -> Self {
        Self::Logical(position)
    }
}

impl From<PhysicalPosition<f64>> for Position {
    fn from(position: PhysicalPosition<f64>) -> Self {
        Self::Physical(position.cast())
    }
}

impl From<LogicalPosition<u32>> for Position {
    fn from(position: LogicalPosition<u32>) -> Self {
        Self::Logical(LogicalPosition::new(position.x.into(), position.y.into()))
    }
}
