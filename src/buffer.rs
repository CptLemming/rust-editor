use std::fs::{File, read_to_string};
use std::io::Write;
use std::ops::Range;

use crate::annotated::AnnotatedString;
use crate::file_info::FileInfo;
use crate::highlighter::Highlighter;
use crate::{line::Line, view::Location};

#[derive(Default)]
pub struct Buffer {
    lines: Vec<Line>,
    file_info: FileInfo,
    dirty: bool,
}

impl Buffer {
    pub fn load(filename: &str) -> anyhow::Result<Self> {
        let contents = read_to_string(filename)?;
        let mut lines = Vec::new();
        for line in contents.lines() {
            lines.push(Line::from(line));
        }
        Ok(Self {
            lines,
            file_info: FileInfo::from(filename),
            dirty: false,
        })
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        self.save_to_file(&self.file_info)?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, filename: &str) -> anyhow::Result<()> {
        let file_info = FileInfo::from(filename);
        self.save_to_file(&file_info)?;
        self.file_info = file_info;
        self.dirty = false;
        Ok(())
    }

    pub fn save_to_file(&self, file_info: &FileInfo) -> anyhow::Result<()> {
        if let Some(path) = &file_info.get_path() {
            let mut file = File::create(path)?;

            for line in &self.lines {
                writeln!(file, "{line}")?;
            }
        }

        Ok(())
    }

    pub fn search_forward(&self, query: &str, from: Location) -> Option<Location> {
        if query.is_empty() {
            return None;
        }

        let mut is_first = true;

        for (line_index, line) in self
            .lines
            .iter()
            .enumerate()
            .cycle()
            .skip(from.line_index)
            .take(self.lines.len().saturating_add(1))
        {
            let from_grapheme_index = if is_first {
                is_first = false;
                from.grapheme_index
            } else {
                0
            };

            if let Some(grapheme_index) = line.search_forward(query, from_grapheme_index) {
                return Some(Location {
                    grapheme_index,
                    line_index,
                });
            }
        }

        None
    }

    pub fn search_backward(&self, query: &str, from: Location) -> Option<Location> {
        if query.is_empty() {
            return None;
        }

        let mut is_first = true;

        for (line_index, line) in self
            .lines
            .iter()
            .enumerate()
            .rev()
            .cycle()
            .skip(
                self.lines
                    .len()
                    .saturating_sub(from.line_index)
                    .saturating_sub(1),
            )
            .take(self.lines.len().saturating_add(1))
        {
            let from_grapheme_index = if is_first {
                is_first = false;
                from.grapheme_index
            } else {
                line.grapheme_count()
            };

            if let Some(grapheme_index) = line.search_backward(query, from_grapheme_index) {
                return Some(Location {
                    grapheme_index,
                    line_index,
                });
            }
        }

        None
    }

    pub fn insert_newline(&mut self, at: &Location) {
        if at.line_index == self.height() {
            self.lines.push(Line::default());
        } else if let Some(line) = self.lines.get_mut(at.line_index) {
            let new = line.split(at.grapheme_index);
            self.lines.insert(at.line_index.saturating_add(1), new);
        }
    }

    pub fn insert_char(&mut self, char: char, at: &Location) {
        if at.line_index > self.height() {
            return;
        }
        if at.line_index == self.height() {
            self.lines.push(Line::from(&char.to_string()));
            self.dirty = true;
        } else if let Some(line) = self.lines.get_mut(at.line_index) {
            line.insert_char(char, at.grapheme_index);
            self.dirty = true;
        }
    }

    pub fn delete(&mut self, at: &Location) {
        if let Some(line) = self.lines.get(at.line_index) {
            if at.grapheme_index >= line.grapheme_count()
                && self.height() > at.line_index.saturating_add(1)
            {
                let next_line = self.lines.remove(at.line_index.saturating_add(1));
                self.lines[at.line_index].append(&next_line);
                self.dirty = true;
            } else if at.grapheme_index < line.grapheme_count() {
                self.lines[at.line_index].delete(at.grapheme_index);
                self.dirty = true;
            }
        }
    }

    pub const fn get_file_info(&self) -> &FileInfo {
        &self.file_info
    }

    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub const fn is_file_loaded(&self) -> bool {
        self.file_info.has_path()
    }

    pub fn height(&self) -> usize {
        self.lines.len()
    }

    pub fn grapheme_count(&self, index: usize) -> usize {
        self.lines.get(index).map_or(0, Line::grapheme_count)
    }

    pub fn width_until(&self, index: usize, grapheme_index: usize) -> usize {
        self.lines
            .get(index)
            .map_or(0, |line| line.width_until(grapheme_index))
    }

    pub fn get_highlighted_substring(
        &self,
        line_index: usize,
        range: Range<usize>,
        highlighter: &Highlighter,
    ) -> Option<AnnotatedString> {
        self.lines.get(line_index).map(|line| {
            line.get_annotated_visible_substr(range, Some(&highlighter.get_annotations(line_index)))
        })
    }

    pub fn highlight(&self, index: usize, highlighter: &mut Highlighter) {
        if let Some(line) = self.lines.get(index) {
            highlighter.highlight(index, line);
        }
    }
}
