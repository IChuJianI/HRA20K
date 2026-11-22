use std::fmt;

/// An error raised by a platform operation.
#[derive(Debug, Clone)]
pub struct OsError {
    code: i32,
    message: String,
}

impl OsError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for OsError {}

/// Reasons `EventLoop::build` or `run_app` can fail.
#[derive(Debug)]
pub enum EventLoopError {
    NotSupported,
    Os(OsError),
    ExitFailure,
    RecreationAttempt,
}

impl fmt::Display for EventLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupported => f.write_str("the operation is not supported"),
            Self::Os(error) => write!(f, "{error}"),
            Self::ExitFailure => f.write_str("the event loop could not start"),
            Self::RecreationAttempt => f.write_str("an event loop already exists"),
        }
    }
}

impl std::error::Error for EventLoopError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Os(error) => Some(error),
            _ => None,
        }
    }
}

impl From<OsError> for EventLoopError {
    fn from(error: OsError) -> Self {
        Self::Os(error)
    }
}

/// An error from creating or using an external object such as a window icon.
#[derive(Debug, Clone)]
pub struct ExternalError {
    message: String,
}

impl ExternalError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl fmt::Display for ExternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ExternalError {}

/// The supplied image data does not match its stated dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadImage;

impl fmt::Display for BadImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the image data does not match the stated dimensions")
    }
}

impl std::error::Error for BadImage {}
