use std::cmp;

use crate::{
    command::Edit,
    line::Line,
    terminal::{Size, Terminal},
    ui_component::UIComponent,
};

#[derive(Default)]
pub struct CommandBar {
    prompt: String,
    value: Line,
    needs_redraw: bool,
    size: Size,
}

impl CommandBar {
    pub fn handle_edit_command(&mut self, command: Edit) {
        match command {
            Edit::Insert(char) => self.value.append_char(char),
            Edit::Delete | Edit::Enter => {}
            Edit::Backspace => self.value.delete_last(),
        }

        self.mark_redraw(true);
    }

    pub fn cursor_position_col(&self) -> usize {
        let max_width = self
            .prompt
            .len()
            .saturating_add(self.value.grapheme_count());
        cmp::min(max_width, self.size.width)
    }

    pub fn value(&self) -> String {
        self.value.to_string()
    }

    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt = prompt.to_string();
        self.mark_redraw(true);
    }

    pub fn clear_value(&mut self) {
        self.value = Line::default();
        self.mark_redraw(true);
    }
}

impl UIComponent for CommandBar {
    fn mark_redraw(&mut self, value: bool) {
        self.needs_redraw = value;
    }

    fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    fn draw(&mut self, origin_y: usize) -> anyhow::Result<()> {
        let area_for_value = self.size.width.saturating_sub(self.prompt.len());
        let value_end = self.value.width();
        let value_start = value_end.saturating_sub(area_for_value);

        let message = format!(
            "{}{}",
            self.prompt,
            self.value.get_visible_graphemes(value_start..value_end)
        );

        let to_print = if message.len() <= self.size.width {
            message
        } else {
            String::new()
        };

        Terminal.print_row(origin_y, &to_print)
    }
}
