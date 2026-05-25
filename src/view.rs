use std::cmp;

use crate::{
    buffer::Buffer,
    command::{Edit, Move},
    document_status::DocumentStatus,
    highlighter::Highlighter,
    line::Line,
    terminal::{Position, Size, Terminal},
    ui_component::UIComponent,
};

pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Default, Clone)]
pub struct Location {
    pub grapheme_index: usize,
    pub line_index: usize,
}

struct SearchInfo {
    prev_location: Location,
    prev_scroll_offset: Position,
    query: Option<Line>,
}

struct CopyInfo {
    start: Location,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum SearchDirection {
    #[default]
    Forward,
    Backward,
}

#[derive(Default)]
pub struct View {
    buffer: Buffer,
    terminal: Terminal,
    text_location: Location,
    scroll_offset: Position,
    needs_redraw: bool,
    size: Size,
    search_info: Option<SearchInfo>,
    copy_info: Option<CopyInfo>,
}

impl View {
    pub const fn is_file_loaded(&self) -> bool {
        self.buffer.is_file_loaded()
    }

    pub fn load(&mut self, filename: &str) -> anyhow::Result<()> {
        let buffer = Buffer::load(filename)?;
        self.buffer = buffer;
        self.mark_redraw(true);
        Ok(())
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        self.buffer.save()?;
        self.mark_redraw(true);
        Ok(())
    }

    pub fn save_as(&mut self, filename: &str) -> anyhow::Result<()> {
        self.buffer.save_as(filename)?;
        self.mark_redraw(true);
        Ok(())
    }

    pub fn get_status(&self) -> DocumentStatus {
        let file_info = self.buffer.get_file_info();
        DocumentStatus {
            total_lines: self.buffer.height(),
            current_line_index: self.text_location.line_index,
            filename: format!("{file_info}"),
            file_type: file_info.get_file_type().clone(),
            is_modified: self.buffer.is_dirty(),
        }
    }

    pub fn caret_position(&self) -> Position {
        self.text_location_to_position()
            .saturating_sub(&self.scroll_offset)
    }

    fn text_location_to_position(&self) -> Position {
        let y = self.text_location.line_index;
        let x = self
            .buffer
            .width_until(y, self.text_location.grapheme_index);
        Position { x, y }
    }

    pub fn handle_edit_command(&mut self, command: Edit) {
        match command {
            Edit::Insert(char) => self.insert_char(char),
            Edit::Delete => self.delete(),
            Edit::Backspace => self.delete_backward(),
            Edit::Enter => self.insert_newline(),
        }
    }

    pub fn handle_move_command(&mut self, command: Move) {
        let Size { height, .. } = self.size;

        match command {
            Move::Up => self.move_up(1),
            Move::Down => self.move_down(1),
            Move::Left => self.move_left(),
            Move::Right => self.move_right(),
            Move::PageUp => self.move_up(height.saturating_sub(1)),
            Move::PageDown => self.move_down(height.saturating_sub(1)),
            Move::Home => self.move_to_start_of_line(),
            Move::End => self.move_to_end_of_line(),
        }
        self.scroll_text_location_into_view();
    }

    fn render_line(&self, at: usize, text: &str) -> anyhow::Result<()> {
        self.terminal.print_row(at, text)
    }

    fn delete(&mut self) {
        self.buffer.delete(&self.text_location);
        self.mark_redraw(true);
    }

    fn delete_backward(&mut self) {
        if self.text_location.line_index != 0 || self.text_location.grapheme_index != 0 {
            self.handle_move_command(Move::Left);
            self.delete();
        }
    }

    fn insert_newline(&mut self) {
        self.buffer.insert_newline(&self.text_location);
        self.handle_move_command(Move::Right);
        self.mark_redraw(true);
    }

    fn insert_char(&mut self, char: char) {
        let old_len = self.buffer.grapheme_count(self.text_location.line_index);
        self.buffer.insert_char(char, &self.text_location);
        let new_len = self.buffer.grapheme_count(self.text_location.line_index);
        let grapheme_delta = new_len.saturating_sub(old_len);
        if grapheme_delta > 0 {
            self.handle_move_command(Move::Right);
        }
        self.mark_redraw(true);
    }

    fn move_up(&mut self, step: usize) {
        self.text_location.line_index = self.text_location.line_index.saturating_sub(step);
        self.snap_to_valid_grapheme();
    }

    fn move_down(&mut self, step: usize) {
        self.text_location.line_index = self.text_location.line_index.saturating_add(step);
        self.snap_to_valid_grapheme();
        self.snap_to_valid_line();
    }

    fn move_right(&mut self) {
        let grapheme_count = self.buffer.grapheme_count(self.text_location.line_index);
        if self.text_location.grapheme_index < grapheme_count {
            self.text_location.grapheme_index += 1;
        } else {
            self.move_to_start_of_line();
            self.move_down(1);
        }
    }

    fn move_left(&mut self) {
        if self.text_location.grapheme_index > 0 {
            self.text_location.grapheme_index -= 1;
        } else if self.text_location.line_index > 0 {
            self.move_up(1);
            self.move_to_end_of_line();
        }
    }

    fn move_to_start_of_line(&mut self) {
        self.text_location.grapheme_index = 0;
    }

    fn move_to_end_of_line(&mut self) {
        self.text_location.grapheme_index =
            self.buffer.grapheme_count(self.text_location.line_index);
    }

    fn snap_to_valid_grapheme(&mut self) {
        self.text_location.grapheme_index = cmp::min(
            self.text_location.grapheme_index,
            self.buffer.grapheme_count(self.text_location.line_index),
        );
    }

    fn snap_to_valid_line(&mut self) {
        self.text_location.line_index =
            cmp::min(self.text_location.line_index, self.buffer.height());
    }

    fn scroll_vertically(&mut self, to: usize) {
        let Size { height, .. } = self.size;
        let offset_changed = if to < self.scroll_offset.y {
            self.scroll_offset.y = to;
            true
        } else if to >= self.scroll_offset.y.saturating_add(height) {
            self.scroll_offset.y = to.saturating_sub(height).saturating_add(1);
            true
        } else {
            false
        };

        if offset_changed {
            self.mark_redraw(true);
        }
    }

    fn scroll_horizontally(&mut self, to: usize) {
        let Size { width, .. } = self.size;
        let offset_changed = if to < self.scroll_offset.x {
            self.scroll_offset.x = to;
            true
        } else if to >= self.scroll_offset.x.saturating_add(width) {
            self.scroll_offset.x = to.saturating_sub(width).saturating_add(1);
            true
        } else {
            false
        };

        if offset_changed {
            self.mark_redraw(true);
        }
    }

    fn scroll_text_location_into_view(&mut self) {
        let Position { x, y } = self.text_location_to_position();
        self.scroll_vertically(y);
        self.scroll_horizontally(x);
    }

    fn center_text_location(&mut self) {
        let Size { height, width } = self.size;
        let Position { x, y } = self.text_location_to_position();
        let vertical_mid = height.div_ceil(2);
        let horizontal_mid = width.div_ceil(2);

        self.scroll_offset.y = y.saturating_sub(vertical_mid);
        self.scroll_offset.x = x.saturating_sub(horizontal_mid);

        self.mark_redraw(true);
    }

    fn build_welcome_message(&self, width: usize) -> String {
        if width == 0 {
            return String::new();
        }

        let message = format!("{NAME} editor -- version {VERSION}");
        let len = message.len();
        let remaining_width = width.saturating_sub(1);

        if remaining_width <= len {
            return "~".to_string();
        }

        format!("{:<1}{:^remaining_width$}", "~", message)
    }

    fn get_search_query(&self) -> Option<&Line> {
        self.search_info
            .as_ref()
            .and_then(|search_info| search_info.query.as_ref())
    }

    pub fn search(&mut self, query: &str) {
        if let Some(search_info) = &mut self.search_info {
            search_info.query = Some(Line::from(query));
        }
        self.search_in_direction(self.text_location.clone(), SearchDirection::default());
    }

    pub fn copy(&self) -> Option<String> {
        if let Some(copy_info) = &self.copy_info {
            return self.buffer.get_substring(
                copy_info.start.line_index,
                copy_info.start.grapheme_index..self.text_location.grapheme_index,
            );
        }

        None
    }

    pub fn paste(&mut self, text: &str) {
        for char in text.chars() {
            match char {
                '\r' => {
                    self.insert_newline();
                }
                _ => {
                    self.insert_char(char);
                }
            }
        }
    }

    pub fn search_in_direction(&mut self, from: Location, direction: SearchDirection) {
        if let Some(location) = self.get_search_query().and_then(|query| {
            if query.is_empty() {
                None
            } else if direction == SearchDirection::Forward {
                self.buffer.search_forward(query, from)
            } else {
                self.buffer.search_backward(query, from)
            }
        }) {
            self.text_location = location;
            self.center_text_location();
        }
        self.mark_redraw(true);
    }

    pub fn search_next(&mut self) {
        let step_right = self
            .get_search_query()
            .map_or(1, |query| cmp::min(query.grapheme_count(), 1));

        let location = Location {
            line_index: self.text_location.line_index,
            grapheme_index: self.text_location.grapheme_index.saturating_add(step_right),
        };
        self.search_in_direction(location, SearchDirection::Forward);
    }

    pub fn search_prev(&mut self) {
        self.search_in_direction(self.text_location.clone(), SearchDirection::Backward);
    }

    pub fn enter_search(&mut self) {
        self.search_info = Some(SearchInfo {
            prev_location: self.text_location.clone(),
            prev_scroll_offset: self.scroll_offset.clone(),
            query: None,
        });
    }

    pub fn exit_search(&mut self) {
        self.search_info = None;
        self.mark_redraw(true);
    }

    pub fn dismiss_search(&mut self) {
        if let Some(search_info) = self.search_info.take() {
            self.text_location = search_info.prev_location;
            self.scroll_offset = search_info.prev_scroll_offset;
            self.scroll_text_location_into_view();
        }

        self.search_info = None;
        self.mark_redraw(true);
    }

    pub fn enter_copy(&mut self) {
        self.copy_info = Some(CopyInfo {
            start: self.text_location.clone(),
        });
    }

    pub fn exit_copy(&mut self) {
        self.copy_info = None;
        self.mark_redraw(true);
    }

    pub fn dismiss_copy(&mut self) {
        self.copy_info = None;
        self.mark_redraw(true);
    }
}

impl UIComponent for View {
    fn mark_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
        self.scroll_text_location_into_view();
    }

    fn draw(&mut self, origin_y: usize) -> anyhow::Result<()> {
        let Size { height, width } = self.size;
        let end_y = origin_y.saturating_add(height);

        let top_third = height.div_ceil(3);
        let scroll_top = self.scroll_offset.y;

        let query = self
            .search_info
            .as_ref()
            .and_then(|search_info| search_info.query.as_deref());
        let selected_match = query.is_some().then_some(self.text_location.clone());
        let copy = self
            .copy_info
            .as_ref()
            .map(|info| (info.start.clone(), self.text_location.clone()));

        let mut highlighter = Highlighter::new(
            query,
            selected_match,
            copy,
            self.buffer.get_file_info().get_file_type(),
        );

        for row in 0..end_y.saturating_add(scroll_top) {
            self.buffer.highlight(row, &mut highlighter);
        }

        for row in origin_y..end_y {
            let line_index = row.saturating_sub(origin_y).saturating_add(scroll_top);

            let left = self.scroll_offset.x;
            let right = self.scroll_offset.x.saturating_add(width);
            if let Some(annotated_string) =
                self.buffer
                    .get_highlighted_substring(line_index, left..right, &highlighter)
            {
                self.terminal.print_annotated_row(row, &annotated_string)?;
            } else if row == top_third && self.buffer.is_empty() {
                self.render_line(row, &self.build_welcome_message(width))?;
            } else {
                self.render_line(row, "~")?;
            }
        }

        self.needs_redraw = true;

        Ok(())
    }
}
