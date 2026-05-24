use std::{cmp, fmt::Display};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AnnotationType {
    Match,
    SelectedMatch,
    Number,
    Keyword,
    Type,
    KnownValue,
    Char,
    LifetimeSpecifier,
    Comment,
    String,
}

#[derive(Debug)]
pub struct AnnotatedStringPart<'a> {
    pub string: &'a str,
    pub annotation_type: Option<AnnotationType>,
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub annotation_type: AnnotationType,
    pub start_byte_index: usize,
    pub end_byte_index: usize,
}

impl Annotation {
    pub fn shift(&mut self, offset: usize) {
        self.start_byte_index = self.start_byte_index.saturating_add(offset);
        self.end_byte_index = self.end_byte_index.saturating_add(offset);
    }
}

#[derive(Debug, Default)]
pub struct AnnotatedString {
    string: String,
    annotations: Vec<Annotation>,
}

impl AnnotatedString {
    pub fn from(string: &str) -> Self {
        Self {
            string: String::from(string),
            annotations: Vec::new(),
        }
    }

    pub fn add_annotation(
        &mut self,
        annotation_type: AnnotationType,
        start_byte_index: usize,
        end_byte_index: usize,
    ) {
        debug_assert!(start_byte_index <= end_byte_index);
        self.annotations.push(Annotation {
            annotation_type,
            start_byte_index,
            end_byte_index,
        });
    }

    pub fn replace(&mut self, start_byte_index: usize, end_byte_index: usize, string: &str) {
        debug_assert!(start_byte_index <= end_byte_index);

        let end_byte_index = cmp::min(end_byte_index, self.string.len());
        if start_byte_index > end_byte_index {
            return;
        }
        self.string
            .replace_range(start_byte_index..end_byte_index, string);

        let replaced_range_len = end_byte_index.saturating_sub(start_byte_index);
        let shortened = string.len() < replaced_range_len;
        let len_difference = string.len().abs_diff(replaced_range_len);

        if len_difference == 0 {
            return;
        }

        self.annotations.iter_mut().for_each(|annotation| {
            annotation.start_byte_index = if annotation.start_byte_index >= end_byte_index {
                if shortened {
                    annotation.start_byte_index.saturating_sub(len_difference)
                } else {
                    annotation.start_byte_index.saturating_add(len_difference)
                }
            } else if annotation.start_byte_index >= start_byte_index {
                if shortened {
                    cmp::max(
                        start_byte_index,
                        annotation.start_byte_index.saturating_sub(len_difference),
                    )
                } else {
                    cmp::min(
                        end_byte_index,
                        annotation.start_byte_index.saturating_add(len_difference),
                    )
                }
            } else {
                annotation.start_byte_index
            };

            annotation.end_byte_index = if annotation.end_byte_index >= end_byte_index {
                if shortened {
                    annotation.end_byte_index.saturating_sub(len_difference)
                } else {
                    annotation.end_byte_index.saturating_add(len_difference)
                }
            } else if annotation.end_byte_index >= start_byte_index {
                if shortened {
                    cmp::max(
                        start_byte_index,
                        annotation.end_byte_index.saturating_sub(len_difference),
                    )
                } else {
                    cmp::min(
                        end_byte_index,
                        annotation.end_byte_index.saturating_add(len_difference),
                    )
                }
            } else {
                annotation.end_byte_index
            };
        });

        self.annotations.retain(|annotation| {
            annotation.start_byte_index < annotation.end_byte_index
                && annotation.start_byte_index < self.string.len()
        });
    }
}

impl Display for AnnotatedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.string)
    }
}

impl<'a> IntoIterator for &'a AnnotatedString {
    type Item = AnnotatedStringPart<'a>;
    type IntoIter = AnnotatedStringIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        AnnotatedStringIterator {
            annotated_string: self,
            current_index: 0,
        }
    }
}

pub struct AnnotatedStringIterator<'a> {
    pub annotated_string: &'a AnnotatedString,
    pub current_index: usize,
}

impl<'a> Iterator for AnnotatedStringIterator<'a> {
    type Item = AnnotatedStringPart<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.annotated_string.string.len() {
            return None;
        }

        if let Some(annotation) = self
            .annotated_string
            .annotations
            .iter()
            .filter(|annotation| {
                annotation.start_byte_index <= self.current_index
                    && annotation.end_byte_index > self.current_index
            })
            .last()
        {
            let end_index = cmp::min(
                annotation.end_byte_index,
                self.annotated_string.string.len(),
            );
            let start_index = self.current_index;
            self.current_index = end_index;
            return Some(AnnotatedStringPart {
                string: &self.annotated_string.string[start_index..end_index],
                annotation_type: Some(annotation.annotation_type.clone()),
            });
        }

        let mut end_index = self.annotated_string.string.len();
        for annotation in &self.annotated_string.annotations {
            if annotation.start_byte_index > self.current_index
                && annotation.start_byte_index < end_index
            {
                end_index = annotation.start_byte_index;
            }
        }

        let start_index = self.current_index;
        self.current_index = end_index;

        Some(AnnotatedStringPart {
            string: &self.annotated_string.string[start_index..end_index],
            annotation_type: None,
        })
    }
}
