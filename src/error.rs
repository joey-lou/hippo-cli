use std::fmt;

/// Expected failures use exit code 2. Anything else is 1.
#[derive(Debug)]
pub enum HippoError {
    Config(String),
    Validation(String),
    NotFound(String),
    Unexpected(String),
}

impl HippoError {
    pub fn unexpected(err: impl fmt::Display) -> Self {
        Self::Unexpected(err.to_string())
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Unexpected(_) => 1,
            _ => 2,
        }
    }
}

impl fmt::Display for HippoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg)
            | Self::Validation(msg)
            | Self::NotFound(msg)
            | Self::Unexpected(msg) => {
                write!(f, "{msg}")
            }
        }
    }
}

impl std::error::Error for HippoError {}

pub type Result<T> = std::result::Result<T, HippoError>;
