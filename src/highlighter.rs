use std::collections::HashMap;

use unicode_segmentation::UnicodeSegmentation;

use crate::{
    annotated::{Annotation, AnnotationType},
    document_status::SupportedFileTypes,
    line::Line,
    view::Location,
};

/// Outer comment
fn create_syntax_highlighter(file_type: &SupportedFileTypes) -> Option<Box<dyn SyntaxHighlighter>> {
    match file_type {
        SupportedFileTypes::Rust => Some(Box::<RustSyntaxHighlighter>::default()),
        SupportedFileTypes::PlainText => None,
    }
}

// Inner comment
pub struct Highlighter<'a> {
    syntax_highlighter: Option<Box<dyn SyntaxHighlighter>>,
    search_result_highlighter: Option<SearchResultHighlighter<'a>>,
}

impl<'a> Highlighter<'a> {
    pub fn new(
        matched_word: Option<&'a str>,
        selected_match: Option<Location>,
        file_type: &SupportedFileTypes,
    ) -> Self {
        let search_result_highlighter = matched_word
            .map(|matched_word| SearchResultHighlighter::new(matched_word, selected_match));

        Self {
            syntax_highlighter: create_syntax_highlighter(file_type),
            search_result_highlighter,
        }
    }

    pub fn get_annotations(&self, index: usize) -> Vec<&Annotation> {
        let mut result = Vec::new();

        if let Some(syntax_highlighter) = &self.syntax_highlighter {
            if let Some(annotations) = syntax_highlighter.get_annotations(index) {
                result.extend(annotations.iter());
            }
        }

        if let Some(search_result_highlighter) = &self.search_result_highlighter {
            if let Some(annotations) = search_result_highlighter.get_annotations(index) {
                result.extend(annotations.iter());
            }
        }

        result
    }

    pub fn highlight(&mut self, index: usize, line: &Line) {
        if let Some(syntax_highlighter) = &mut self.syntax_highlighter {
            syntax_highlighter.highlight(index, line);
        }
        if let Some(search_result_highlighter) = &mut self.search_result_highlighter {
            search_result_highlighter.highlight(index, line);
        }
    }
}

/**
 * Longer comment
 */
pub trait SyntaxHighlighter {
    fn highlight(&mut self, index: usize, line: &Line);
    fn get_annotations(&self, index: usize) -> Option<&Vec<Annotation>>;
}

/**
 * /*
 * Nested comments
 * */
 */
#[derive(Default)]
pub struct SearchResultHighlighter<'a> {
    matched_word: &'a str,
    selected_match: Option<Location>,
    highlights: HashMap<usize, Vec<Annotation>>,
}

impl<'a> SearchResultHighlighter<'a> {
    pub fn new(matched_word: &'a str, selected_match: Option<Location>) -> Self {
        Self {
            matched_word,
            selected_match,
            highlights: HashMap::new(),
        }
    }

    fn highlight_matched_words(&self, line: &Line, result: &mut Vec<Annotation>) {
        if self.matched_word.is_empty() {
            return;
        }

        line.find_all(self.matched_word, 0..line.len())
            .iter()
            .for_each(|(start, _)| {
                result.push(Annotation {
                    annotation_type: AnnotationType::Match,
                    start_byte_index: *start,
                    end_byte_index: start.saturating_add(self.matched_word.len()),
                });
            });
    }

    fn highlight_selected_match(&self, result: &mut Vec<Annotation>) {
        if let Some(selected_match) = &self.selected_match {
            if self.matched_word.is_empty() {
                return;
            }

            let start = selected_match.grapheme_index;
            result.push(Annotation {
                annotation_type: AnnotationType::SelectedMatch,
                start_byte_index: start,
                end_byte_index: start.saturating_add(self.matched_word.len()),
            });
        }
    }
}

impl<'a> SyntaxHighlighter for SearchResultHighlighter<'a> {
    fn highlight(&mut self, index: usize, line: &Line) {
        let mut result = Vec::new();
        self.highlight_matched_words(line, &mut result);

        if let Some(selected_match) = &self.selected_match {
            if selected_match.line_index == index {
                self.highlight_selected_match(&mut result);
            }
        }

        self.highlights.insert(index, result);
    }

    fn get_annotations(&self, index: usize) -> Option<&Vec<Annotation>> {
        self.highlights.get(&index)
    }
}

#[derive(Default)]
pub struct RustSyntaxHighlighter {
    highlights: Vec<Vec<Annotation>>,
    ml_comment_balance: usize,
    in_ml_string: bool,
}

impl RustSyntaxHighlighter {
    fn annotate_ml_comment(&mut self, string: &str) -> Option<Annotation> {
        let mut chars = string.char_indices().peekable();

        while let Some((_, char)) = chars.next() {
            if char == '/' {
                if let Some((_, '*')) = chars.peek() {
                    self.ml_comment_balance = self.ml_comment_balance.saturating_add(1);
                    chars.next();
                }
            } else if self.ml_comment_balance == 0 {
                return None;
            } else if char == '*' {
                if let Some((index, '/')) = chars.peek() {
                    self.ml_comment_balance = self.ml_comment_balance.saturating_sub(1);

                    if self.ml_comment_balance == 0 {
                        return Some(Annotation {
                            annotation_type: AnnotationType::Comment,
                            start_byte_index: 0,
                            end_byte_index: index.saturating_add(1),
                        });
                    }

                    chars.next();
                }
            }
        }

        (self.ml_comment_balance > 0).then_some(Annotation {
            annotation_type: AnnotationType::Comment,
            start_byte_index: 0,
            end_byte_index: string.len(),
        })
    }

    fn annotate_string(&mut self, string: &str) -> Option<Annotation> {
        let mut chars = string.char_indices();

        while let Some((index, char)) = chars.next() {
            if char == '\\' && self.in_ml_string {
                chars.next();
                continue;
            }

            if char == '"' {
                if self.in_ml_string {
                    self.in_ml_string = false;
                    return Some(Annotation {
                        annotation_type: AnnotationType::String,
                        start_byte_index: 0,
                        end_byte_index: index.saturating_add(1),
                    });
                }

                self.in_ml_string = true;
            }

            if !self.in_ml_string {
                return None;
            }
        }

        self.in_ml_string.then_some(Annotation {
            annotation_type: AnnotationType::String,
            start_byte_index: 0,
            end_byte_index: string.len(),
        })
    }

    fn initial_annotation(&mut self, line: &Line) -> Option<Annotation> {
        if self.in_ml_string {
            self.annotate_string(line)
        } else if self.ml_comment_balance > 0 {
            self.annotate_ml_comment(line)
        } else {
            None
        }
    }

    fn annotate_remainder(&mut self, remainder: &str) -> Option<Annotation> {
        self.annotate_ml_comment(remainder)
            .or_else(|| self.annotate_string(remainder))
            .or_else(|| annotate_single_line_comment(remainder))
            .or_else(|| annotate_char(remainder))
            .or_else(|| annotate_lifetime_specifier(remainder))
            .or_else(|| annotate_number(remainder))
            .or_else(|| annotate_keyword(remainder))
            .or_else(|| annotate_type(remainder))
            .or_else(|| annotate_known_value(remainder))
    }
}

impl SyntaxHighlighter for RustSyntaxHighlighter {
    fn highlight(&mut self, index: usize, line: &Line) {
        debug_assert_eq!(index, self.highlights.len());

        let mut result = Vec::new();
        let mut iterator = line.split_word_bound_indices().peekable();

        if let Some(annotation) = self.initial_annotation(line) {
            result.push(annotation.clone());

            while let Some(&(next_index, _)) = iterator.peek() {
                if next_index >= annotation.end_byte_index {
                    break;
                }
                iterator.next();
            }
        }

        while let Some((start_index, _)) = iterator.next() {
            let remainder = &line[start_index..];

            if let Some(mut annotation) = self.annotate_remainder(remainder) {
                annotation.shift(start_index);
                result.push(annotation.clone());

                while let Some(&(next_index, _)) = iterator.peek() {
                    if next_index >= annotation.end_byte_index {
                        break;
                    }
                    iterator.next();
                }
            }
        }

        self.highlights.push(result);
    }

    fn get_annotations(&self, index: usize) -> Option<&Vec<Annotation>> {
        self.highlights.get(index)
    }
}

fn annotate_next_word<F>(
    string: &str,
    annotation_type: AnnotationType,
    validator: F,
) -> Option<Annotation>
where
    F: Fn(&str) -> bool,
{
    let mut iter = string.split_word_bounds().peekable();

    if let Some(word) = iter.next() {
        if validator(word) {
            return Some(Annotation {
                annotation_type,
                start_byte_index: 0,
                end_byte_index: word.len(),
            });
        }
    }
    None
}

fn annotate_single_line_comment(string: &str) -> Option<Annotation> {
    if string.starts_with("//") {
        return Some(Annotation {
            annotation_type: AnnotationType::Comment,
            start_byte_index: 0,
            end_byte_index: string.len(),
        });
    }

    None
}

fn annotate_lifetime_specifier(string: &str) -> Option<Annotation> {
    let mut iter = string.split_word_bound_indices();

    if let Some((_, "\'")) = iter.next() {
        if let Some((index, next_word)) = iter.next() {
            return Some(Annotation {
                annotation_type: AnnotationType::LifetimeSpecifier,
                start_byte_index: 0,
                end_byte_index: index.saturating_add(next_word.len()),
            });
        }
    }

    None
}

fn annotate_number(string: &str) -> Option<Annotation> {
    annotate_next_word(string, AnnotationType::Number, is_valid_number)
}

fn annotate_type(string: &str) -> Option<Annotation> {
    annotate_next_word(string, AnnotationType::Type, is_type)
}

fn annotate_keyword(string: &str) -> Option<Annotation> {
    annotate_next_word(string, AnnotationType::Keyword, is_keyword)
}

fn annotate_known_value(string: &str) -> Option<Annotation> {
    annotate_next_word(string, AnnotationType::KnownValue, is_known_value)
}

fn annotate_char(string: &str) -> Option<Annotation> {
    let mut iter = string.split_word_bound_indices().peekable();

    if let Some((_, "\'")) = iter.next() {
        if let Some((_, "\\")) = iter.peek() {
            iter.next();
        }
        iter.next();

        if let Some((index, "\'")) = iter.next() {
            return Some(Annotation {
                annotation_type: AnnotationType::Char,
                start_byte_index: 0,
                end_byte_index: index.saturating_add(1),
            });
        }
    }

    None
}

fn is_valid_number(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    if is_numeric_literal(word) {
        return true;
    }

    let mut chars = word.chars();

    if let Some(first_char) = chars.next() {
        if !first_char.is_ascii_digit() {
            return false;
        }
    }

    let mut seen_dot = false;
    let mut seen_e = false;
    let mut prev_was_digit = true;

    for char in chars {
        match char {
            '0'..='9' => {
                prev_was_digit = true;
            }
            '_' => {
                if !prev_was_digit {
                    return false;
                }
                prev_was_digit = false;
            }
            '.' => {
                if seen_dot || seen_e || !prev_was_digit {
                    return false;
                }
                seen_dot = true;
                prev_was_digit = false;
            }
            'e' | 'E' => {
                if seen_e || !prev_was_digit {
                    return false;
                }
                seen_e = true;
                prev_was_digit = false;
            }
            _ => {
                return false;
            }
        }
    }

    prev_was_digit
}

fn is_numeric_literal(word: &str) -> bool {
    if word.len() < 3 {
        return false;
    }

    let mut chars = word.chars();

    if chars.next() != Some('0') {
        return false;
    }

    let base = match chars.next() {
        Some('b' | 'B') => 2,
        Some('o' | 'O') => 8,
        Some('x' | 'X') => 16,
        _ => return false,
    };

    chars.all(|char| char.is_digit(base))
}

fn is_keyword(word: &str) -> bool {
    KEYWORDS.contains(&word)
}

fn is_type(word: &str) -> bool {
    TYPES.contains(&word)
}

fn is_known_value(word: &str) -> bool {
    KNOWN_VALUES.contains(&word)
}

const KEYWORDS: [&str; 54] = [
    "break",
    "const",
    "continue",
    "crate",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "async",
    "await",
    "dyn",
    "abstract",
    "become",
    "box",
    "do",
    "final",
    "macro",
    "override",
    "priv",
    "typeof",
    "unsized",
    "virtual",
    "yield",
    "try",
    "macro_rules",
    "union",
    "format",
    "panic",
];

const TYPES: [&str; 22] = [
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32",
    "f64", "bool", "char", "Option", "Result", "String", "str", "Vec", "HashMap",
];

const KNOWN_VALUES: [&str; 6] = ["Some", "None", "true", "false", "Ok", "Err"];
