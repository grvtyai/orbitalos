use std::error::Error;
use std::fmt;
use std::time::SystemTimeError;

pub type OrbitalResult<T> = Result<T, OrbitalError>;

#[derive(Debug)]
pub enum OrbitalError {
    DataInvariant(&'static str),
    Database(rusqlite::Error),
    MissingHomeDirectory,
    Io(std::io::Error),
    NotFound {
        entity: &'static str,
        id: String,
    },
    Time(SystemTimeError),
}

impl fmt::Display for OrbitalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataInvariant(message) => write!(f, "data invariant violated: {message}"),
            Self::Database(error) => write!(f, "database error: {error}"),
            Self::MissingHomeDirectory => {
                f.write_str("could not determine the current user's home directory")
            }
            Self::Io(error) => write!(f, "i/o error: {error}"),
            Self::NotFound { entity, id } => write!(f, "{entity} with id '{id}' was not found"),
            Self::Time(error) => write!(f, "system time error: {error}"),
        }
    }
}

impl Error for OrbitalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DataInvariant(_) => None,
            Self::Database(error) => Some(error),
            Self::MissingHomeDirectory => None,
            Self::Io(error) => Some(error),
            Self::NotFound { .. } => None,
            Self::Time(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for OrbitalError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for OrbitalError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

impl From<SystemTimeError> for OrbitalError {
    fn from(value: SystemTimeError) -> Self {
        Self::Time(value)
    }
}
