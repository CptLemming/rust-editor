use std::{
    fmt,
    ops::{Deref, Range},
};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::annotated::{AnnotatedString, Annotation};

#[derive(Debug, Clone)]
enum GraphemeWidth {
    Half,
    Full,
}

impl From<GraphemeWidth> for usize {
    fn from(value: GraphemeWidth) -> Self {
        match value {
            GraphemeWidth::Half => 1,
            GraphemeWidth::Full => 2,
        }
    }
}

struct TextFragment {
    grapheme: String,
    rendered_width: GraphemeWidth,
    replacement: Option<char>,
    start_byte_index: usize,
}

#[derive(Default)]
pub struct Line {
    fragments: Vec<TextFragment>,
    string: String,
}

impl Line {
    pub fn from(line: &str) -> Self {
        debug_assert!(line.is_empty() || line.lines().count() == 1);

        let fragments = Self::str_to_fragments(line);
        Self {
            fragments,
            string: String::from(line),
        }
    }

    fn str_to_fragments(line: &str) -> Vec<TextFragment> {
        line.grapheme_indices(true)
            .map(|(byte_index, grapheme)| {
                let (replacement, rendered_width) = Self::replacement_character(grapheme)
                    .map_or_else(
                        || {
                            let unicode_width = grapheme.width();
                            let rendered_width = match unicode_width {
                                0 | 1 => GraphemeWidth::Half,
                                _ => GraphemeWidth::Full,
                            };
                            (None, rendered_width)
                        },
                        |replacement| (Some(replacement), GraphemeWidth::Half),
                    );

                TextFragment {
                    grapheme: grapheme.to_string(),
                    rendered_width,
                    replacement,
                    start_byte_index: byte_index,
                }
            })
            .collect()
    }

    fn rebuild_fragments(&mut self) {
        self.fragments = Self::str_to_fragments(&self.string);
    }

    fn replacement_character(for_str: &str) -> Option<char> {
        let width = for_str.width();
        match for_str {
            " " => None,
            "\t" => Some(' '),
            _ if width > 0 && for_str.trim().is_empty() => Some('␣'),
            _ if width == 0 => {
                let mut chars = for_str.chars();
                if let Some(ch) = chars.next() {
                    if ch.is_control() && chars.next().is_none() {
                        return Some('▯');
                    }
                }
                Some('·')
            }
            _ => None,
        }
    }

    pub fn search_forward(&self, query: &str, from_grapheme_index: usize) -> Option<usize> {
        debug_assert!(from_grapheme_index <= self.grapheme_count());

        if from_grapheme_index == self.grapheme_count() {
            return None;
        }

        let start_byte_index = self.grapheme_index_to_byte_index(from_grapheme_index);

        self.find_all(query, start_byte_index..self.string.len())
            .first()
            .map(|(_, grapheme_index)| *grapheme_index)
    }

    pub fn search_backward(&self, query: &str, from_grapheme_index: usize) -> Option<usize> {
        debug_assert!(from_grapheme_index <= self.grapheme_count());

        if from_grapheme_index == 0 {
            return None;
        }

        let end_byte_index = if from_grapheme_index == self.grapheme_count() {
            self.string.len()
        } else {
            self.grapheme_index_to_byte_index(from_grapheme_index)
        };

        self.find_all(query, 0..end_byte_index)
            .last()
            .map(|(_, grapheme_index)| *grapheme_index)
    }

    pub fn get_visible_graphemes(&self, range: Range<usize>) -> String {
        self.get_annotated_visible_substr(range, None).to_string()
    }

    pub fn get_annotated_visible_substr(
        &self,
        range: Range<usize>,
        annotations: Option<&Vec<&Annotation>>,
    ) -> AnnotatedString {
        if range.start >= range.end {
            return AnnotatedString::default();
        }

        let mut result = AnnotatedString::from(&self.string);

        if let Some(annotations) = annotations {
            for annotation in annotations {
                result.add_annotation(
                    annotation.annotation_type.clone(),
                    annotation.start_byte_index,
                    annotation.end_byte_index,
                );
            }
        }

        let mut fragment_start = self.width();
        for fragment in self.fragments.iter().rev() {
            let fragment_end = fragment_start;
            fragment_start = fragment_start.saturating_sub(fragment.rendered_width.clone().into());

            if fragment_start > range.end {
                continue;
            }

            if fragment_start < range.end && fragment_end > range.end {
                result.replace(fragment.start_byte_index, self.string.len(), "⋯");
                continue;
            } else if fragment_start == range.end {
                result.replace(fragment.start_byte_index, self.string.len(), "");
                continue;
            }

            if fragment_end <= range.start {
                result.replace(
                    0,
                    fragment
                        .start_byte_index
                        .saturating_add(fragment.grapheme.len()),
                    "",
                );
                break;
            } else if fragment_start < range.start && fragment_end > range.start {
                result.replace(
                    0,
                    fragment
                        .start_byte_index
                        .saturating_add(fragment.grapheme.len()),
                    "⋯",
                );
                break;
            }

            if fragment_start >= range.start && fragment_end <= range.end {
                if let Some(replacement) = fragment.replacement {
                    let start_byte_index = fragment.start_byte_index;
                    let end_byte_index = start_byte_index.saturating_add(fragment.grapheme.len());
                    result.replace(start_byte_index, end_byte_index, &replacement.to_string());
                }
            }
        }

        result
    }

    pub fn insert_char(&mut self, char: char, grapheme_index: usize) {
        debug_assert!(grapheme_index.saturating_sub(1) <= self.grapheme_count());

        if let Some(fragment) = self.fragments.get(grapheme_index) {
            self.string.insert(fragment.start_byte_index, char);
        } else {
            self.string.push(char);
        }
        self.rebuild_fragments();
    }

    pub fn split(&mut self, grapheme_index: usize) -> Self {
        if let Some(fragment) = self.fragments.get(grapheme_index) {
            let remainder = self.string.split_off(fragment.start_byte_index);
            self.rebuild_fragments();
            Self::from(&remainder)
        } else {
            Self::default()
        }
    }

    pub fn append(&mut self, other: &Self) {
        self.string.push_str(&other.string);
        self.rebuild_fragments();
    }

    pub fn append_char(&mut self, char: char) {
        self.insert_char(char, self.grapheme_count());
    }

    pub fn delete(&mut self, grapheme_index: usize) {
        debug_assert!(grapheme_index <= self.grapheme_count());

        if let Some(fragment) = self.fragments.get(grapheme_index) {
            let start = fragment.start_byte_index;
            let end = fragment
                .start_byte_index
                .saturating_add(fragment.grapheme.len());
            self.string.drain(start..end);
            self.rebuild_fragments();
        }
    }

    pub fn delete_last(&mut self) {
        self.delete(self.grapheme_count().saturating_sub(1));
    }

    fn byte_index_to_grapheme_index(&self, byte_index: usize) -> Option<usize> {
        if byte_index > self.string.len() {
            return None;
        }
        self.fragments
            .iter()
            .position(|fragment| fragment.start_byte_index >= byte_index)
    }

    fn grapheme_index_to_byte_index(&self, grapheme_index: usize) -> usize {
        debug_assert!(grapheme_index <= self.grapheme_count());

        if grapheme_index == 0 || self.grapheme_count() == 0 {
            return 0;
        }

        self.fragments
            .get(grapheme_index)
            .map_or(0, |fragment| fragment.start_byte_index)
    }

    pub fn grapheme_count(&self) -> usize {
        self.fragments.len()
    }

    pub fn find_all(&self, query: &str, range: Range<usize>) -> Vec<(usize, usize)> {
        let end_byte_index = range.end;
        let start_byte_index = range.start;

        self.string
            .get(start_byte_index..end_byte_index)
            .map_or_else(Vec::new, |substr| {
                substr
                    .match_indices(query)
                    .filter_map(|(relative_start_index, _)| {
                        let absolute_start_index =
                            relative_start_index.saturating_add(start_byte_index);

                        self.byte_index_to_grapheme_index(absolute_start_index)
                            .map(|grapheme_index| (absolute_start_index, grapheme_index))
                    })
                    .collect()
            })
    }

    pub fn width(&self) -> usize {
        self.width_until(self.grapheme_count())
    }

    pub fn width_until(&self, grapheme_index: usize) -> usize {
        self.fragments
            .iter()
            .take(grapheme_index)
            .map(|fragment| match fragment.rendered_width {
                GraphemeWidth::Half => 1,
                GraphemeWidth::Full => 2,
            })
            .sum()
    }
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.string)
    }
}

impl Deref for Line {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}
