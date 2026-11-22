//! Monitor descriptions.

use crate::dpi::{PhysicalPosition, PhysicalSize};

/// A video mode a monitor can present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoMode {
    pub size: PhysicalSize<u32>,
    pub bit_depth: u16,
    pub refresh_rate_millihertz: u32,
}

/// One display attached to the system.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonitorHandle {
    name: Option<String>,
    size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    scale_factor_bits: u64,
    refresh_rate_millihertz: Option<u32>,
}

impl MonitorHandle {
    pub(crate) fn new(
        name: Option<String>,
        size: PhysicalSize<u32>,
        position: PhysicalPosition<i32>,
        scale_factor: f64,
        refresh_rate_millihertz: Option<u32>,
    ) -> Self {
        Self {
            name,
            size,
            position,
            scale_factor_bits: scale_factor.to_bits(),
            refresh_rate_millihertz,
        }
    }

    pub fn name(&self) -> Option<String> {
        self.name.clone()
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        self.position
    }

    pub fn scale_factor(&self) -> f64 {
        f64::from_bits(self.scale_factor_bits)
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        self.refresh_rate_millihertz
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        std::iter::once(VideoMode {
            size: self.size,
            bit_depth: 32,
            refresh_rate_millihertz: self.refresh_rate_millihertz.unwrap_or(60_000),
        })
    }
}
