use std::error::Error;
use std::fmt;

pub type OrbitalResult<T> = Result<T, OrbitalError>;

#[derive(Debug)]
pub enum OrbitalError {
    MissingHomeDirectory,
    Io(std::io::Error),
}

impl fmt::Display for OrbitalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHomeDirectory => {
                f.write_str("could not determine the current user's home directory")
            }
            Self::Io(error) => write!(f, "i/o error: {error}"),
        }
    }
}

impl Error for OrbitalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingHomeDirectory => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for OrbitalError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

