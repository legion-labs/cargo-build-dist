use std::fmt::Display;

#[derive(thiserror::Error, Debug)]
pub struct Error {
    description: String,
    explanation: Option<String>,
    #[source]
    source: Option<anyhow::Error>,
    output: Option<String>,
}

impl Error {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            explanation: None,
            source: None,
            output: None,
        }
    }

    pub fn from_source(source: impl Into<anyhow::Error>) -> Self {
        Self::new("").with_source(source)
    }

    pub fn with_source(mut self, source: impl Into<anyhow::Error>) -> Self {
        self.source = Some(source.into());

        self
    }

    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());

        self
    }

    pub fn with_output(mut self, output: impl Into<String>) -> Self {
        self.output = Some(output.into());

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

    pub fn output(&self) -> Option<&str> {
        self.output.as_deref()
    }

    pub fn with_context(mut self, description: impl Into<String>) -> Self {
        if self.description.is_empty() {
            self.description = description.into();

            self
        } else {
            Self::new(description).with_source(self)
        }
    }
}

pub trait ErrorContext {
    fn with_context(self, description: impl Into<String>) -> Self;
    fn with_full_context(
        self,
        description: impl Into<String>,
        explanation: impl Into<String>,
    ) -> Self;
}

impl<T> ErrorContext for Result<T> {
    fn with_context(self, description: impl Into<String>) -> Self {
        self.map_err(|e| e.with_context(description))
    }

    fn with_full_context(
        self,
        description: impl Into<String>,
        explanation: impl Into<String>,
    ) -> Self {
        self.map_err(|e| e.with_context(description).with_explanation(explanation))
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
