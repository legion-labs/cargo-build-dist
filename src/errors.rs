use std::fmt::Display;

#[derive(thiserror::Error, Debug)]
pub struct Error {
    description: String,
    explanation: Option<String>,
    #[source]
    source: Option<anyhow::Error>,
}

impl Error {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            explanation: None,
            source: None,
        }
    }

    pub fn new_from_source(
        description: impl Into<String>,
        source: impl Into<anyhow::Error>,
    ) -> Self {
        Self {
            description: description.into(),
            explanation: None,
            source: Some(source.into()),
        }
    }

    pub fn new_with_explanation(description: impl Into<String>, explanation: String) -> Self {
        Self {
            description: description.into(),
            explanation: Some(explanation),
            source: None,
        }
    }

    pub fn new_from_source_with_explanation(
        description: impl Into<String>,
        source: impl Into<anyhow::Error>,
        explanation: String,
    ) -> Self {
        Self {
            description: description.into(),
            explanation: Some(explanation),
            source: Some(source.into()),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description)?;

        if let Some(explanation) = &self.explanation {
            write!(f, "\n\n{}", explanation)?;
        }

        Ok(())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[macro_export]
macro_rules! bail {
    ($msg:literal $(,)?) => {
        return Err($crate::Error::new($msg))
    };
    ($err:expr $(,)?) => {
        return Err($crate::Error::new($err))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err($crate::Error::new(format!($fmt, $($arg)*)))
    };
}
