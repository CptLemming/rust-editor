use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::document_status::SupportedFileTypes;

#[derive(Debug, Default)]
pub struct FileInfo {
    path: Option<PathBuf>,
    file_type: SupportedFileTypes,
}

impl FileInfo {
    pub fn from(filename: &str) -> Self {
        let path = PathBuf::from(filename);
        let file_type = if path
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("rs"))
        {
            SupportedFileTypes::Rust
        } else {
            SupportedFileTypes::PlainText
        };

        Self {
            path: Some(path),
            file_type,
        }
    }

    pub fn get_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn get_file_type(&self) -> &SupportedFileTypes {
        &self.file_type
    }

    pub const fn has_path(&self) -> bool {
        self.path.is_some()
    }
}

impl fmt::Display for FileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self
            .get_path()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("[No name]");
        write!(f, "{name}")
    }
}
