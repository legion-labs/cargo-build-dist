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

    pub fn with_source(mut self, source: impl Into<anyhow::Error>) -> Self {
        self.source = Some(source.into());

        self
    }

    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());

        self
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn source(&self) -> Option<&anyhow::Error> {
        self.source.as_ref()
    }

    pub fn explanation(&self) -> Option<&str> {
        self.explanation.as_deref()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description)?;

        if let Some(source) = self.source.as_ref() {
            write!(f, ": {}", source)?;
        }

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
