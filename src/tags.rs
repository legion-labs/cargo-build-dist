use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Tags {
    #[serde(default)]
    pub versions: BTreeMap<cargo_metadata::Version, String>,
}

impl Tags {
    pub fn read_file(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .map_err(|err| Error::new("failed to open tags file").with_source(err))?;

        Self::read(file)
    }

    pub fn read(mut r: impl Read) -> Result<Self> {
        let mut data = String::new();

        r.read_to_string(&mut data)
            .map_err(|err| Error::new("failed to read tags").with_source(err))?;

        let tags = toml::from_str(&data)
            .map_err(|err| Error::new("failed to decode tags").with_source(err))?;

        Ok(tags)
    }

    pub fn write_file(&self, path: &Path) -> Result<()> {
        let file = File::create(path)
            .map_err(|err| Error::new("failed to open tags file").with_source(err))?;

        self.write(file)
    }

    pub fn write(&self, mut w: impl Write) -> Result<()> {
        let data = toml::to_string_pretty(&self)
            .map_err(|err| Error::new("failed to encode tags").with_source(err))?;

        w.write_all(data.as_bytes())
            .map_err(|err| Error::new("failed to write tags").with_source(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_file() {
        let tags = Tags::read_file(Path::new("tests/fixtures/tags.toml")).unwrap();

        assert_eq!(tags.versions.len(), 2);
        assert_eq!(
            tags.versions[&cargo_metadata::Version::new(0, 1, 0)],
            "abcd"
        );
        assert_eq!(
            tags.versions[&cargo_metadata::Version::new(0, 2, 0)],
            "efgh"
        );
    }
}
