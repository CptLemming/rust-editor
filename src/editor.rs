use std::{env, panic};

use crate::command::Command;
use crate::command::Edit;
use crate::command::Move;
use crate::command::System;
use crate::command_bar::CommandBar;
use crate::message_bar::MessageBar;
use crate::status_bar::StatusBar;
use crate::terminal::Position;
use crate::terminal::Size;
use crate::terminal::Terminal;
use crate::ui_component::UIComponent;
use crate::view::{NAME, View};
use crossterm::event::{Event, KeyEvent, KeyEventKind, read};

const QUIT_TIMES: u8 = 3;

#[derive(Debug, Default, PartialEq, Eq)]
enum PromptType {
    Search,
    Save,
    #[default]
    None,
}

impl PromptType {
    fn is_none(&self) -> bool {
        *self == Self::None
    }
}

#[derive(Default)]
pub struct Editor {
    terminal: Terminal,
    view: View,
    should_quit: bool,
    status_bar: StatusBar,
    message_bar: MessageBar,
    command_bar: CommandBar,
    prompt_type: PromptType,
    terminal_size: Size,
    title: String,
    quit_times: u8,
}

impl Editor {
    pub fn new() -> anyhow::Result<Self> {
        let current_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            let _ = Terminal.terminate();
            current_hook(panic_info);
        }));

        Terminal.init()?;

        let mut editor = Self::default();
        let size = Terminal.size().unwrap_or_default();

        editor.handle_resize_command(size);
        editor.update_message("HELP: Ctrl-F find | Ctrl-S = save | Ctrl-Q = quit");

        let args: Vec<String> = env::args().collect();

        if let Some(filename) = args.get(1) {
            if editor.view.load(filename).is_err() {
                editor.update_message(&format!("ERR: Could not open file : {filename}"));
            }
        }

        editor.refresh_status();

        Ok(editor)
    }

    pub fn run(&mut self) {
        loop {
            self.refresh();

            if self.should_quit {
                break;
            }

            match read() {
                Ok(event) => {
                    self.eval(event);
                }
                Err(err) => {
                    #[cfg(debug_assertions)]
                    {
                        panic!("Count not read event: {err:?}");
                    }
                }
            }

            self.refresh_status();
        }
    }

    fn eval(&mut self, event: Event) {
        let should_process = match &event {
            Event::Key(KeyEvent { kind, .. }) => kind == &KeyEventKind::Press,
            Event::Resize(_, _) => true,
            _ => false,
        };

        if !should_process {
            return;
        }

        if let Ok(command) = Command::try_from(event) {
            self.process_command(command);
        }
    }

    fn process_command(&mut self, command: Command) {
        if let Command::System(System::Resize(size)) = command {
            self.handle_resize_command(size);
            return;
        }

        match self.prompt_type {
            PromptType::Search => self.process_command_search_prompt(command),
            PromptType::Save => self.process_command_save_prompt(command),
            PromptType::None => self.process_command_no_prompt(command),
        }
    }

    fn process_command_no_prompt(&mut self, command: Command) {
        if matches!(command, Command::System(System::Quit)) {
            self.handle_quit_command();
            return;
        }
        self.reset_quit_times();

        match command {
            Command::System(System::Quit | System::Resize(_) | System::Dismiss) => {}
            Command::System(System::Search) => self.set_prompt(PromptType::Search),
            Command::System(System::Save) => self.handle_save_command(),
            Command::Edit(edit_command) => self.view.handle_edit_command(edit_command),
            Command::Move(move_command) => self.view.handle_move_command(move_command),
        }
    }

    fn process_command_save_prompt(&mut self, command: Command) {
        match command {
            Command::System(System::Quit | System::Resize(_) | System::Search | System::Save)
            | Command::Move(_) => {}
            Command::System(System::Dismiss) => {
                self.set_prompt(PromptType::None);
                self.update_message("Save aborted");
            }
            Command::Edit(Edit::Enter) => {
                let filename = self.command_bar.value();
                self.save(Some(&filename));
                self.set_prompt(PromptType::None);
            }
            Command::Edit(edit_command) => self.command_bar.handle_edit_command(edit_command),
        }
    }

    fn process_command_search_prompt(&mut self, command: Command) {
        match command {
            Command::System(System::Dismiss) => {
                self.set_prompt(PromptType::None);
                self.view.dismiss_search();
            }
            Command::Edit(Edit::Enter) => {
                self.set_prompt(PromptType::None);
                self.view.exit_search();
            }
            Command::Edit(edit_command) => {
                self.command_bar.handle_edit_command(edit_command);
                let query = self.command_bar.value();
                self.view.search(&query);
            }
            Command::Move(Move::Right | Move::Down) => self.view.search_next(),
            Command::Move(Move::Up | Move::Left) => self.view.search_prev(),
            Command::System(System::Quit | System::Resize(_) | System::Search | System::Save)
            | Command::Move(_) => {}
        }
    }

    fn in_prompt(&mut self) -> bool {
        !self.prompt_type.is_none()
    }

    fn set_prompt(&mut self, prompt: PromptType) {
        match prompt {
            PromptType::None => self.message_bar.mark_redraw(true),
            PromptType::Save => self.command_bar.set_prompt("Save as: "),
            PromptType::Search => {
                self.view.enter_search();
                self.command_bar
                    .set_prompt("Search (Esc to cancel, arrows to navigate): ");
            }
        }
        self.command_bar.clear_value();
        self.prompt_type = prompt;
    }

    fn handle_save_command(&mut self) {
        if self.view.is_file_loaded() {
            self.save(None);
        } else {
            self.set_prompt(PromptType::Save);
        }
    }

    fn save(&mut self, filename: Option<&str>) {
        let result = if let Some(name) = filename {
            self.view.save_as(name)
        } else {
            self.view.save()
        };

        if result.is_ok() {
            self.update_message("File saved successfully");
        } else {
            self.update_message("Error writing file");
        }
    }

    fn handle_quit_command(&mut self) {
        if !self.view.get_status().is_modified || self.quit_times + 1 == QUIT_TIMES {
            self.should_quit = true;
        } else if self.view.get_status().is_modified {
            self.update_message(&format!(
                "WARN: File has unsaved changes. Press quit {} more times to quit",
                QUIT_TIMES - self.quit_times - 1
            ));

            self.quit_times += 1;
        }
    }

    fn reset_quit_times(&mut self) {
        if self.quit_times > 0 {
            self.quit_times = 0;
            self.update_message("");
        }
    }

    fn handle_resize_command(&mut self, size: Size) {
        self.view.resize(Size {
            height: size.height.saturating_sub(2),
            width: size.width,
        });
        let bar_size = Size {
            height: 1,
            width: size.width,
        };
        self.message_bar.resize(bar_size.clone());
        self.status_bar.resize(bar_size.clone());
        self.command_bar.resize(bar_size.clone());
        self.terminal_size = size;
    }

    fn refresh(&mut self) {
        if self.terminal_size.height == 0 || self.terminal_size.width == 0 {
            return;
        }

        let bottom_bar_row = self.terminal_size.height.saturating_sub(1);

        let _ = self.terminal.hide_cursor();

        if self.in_prompt() {
            self.command_bar.render(bottom_bar_row);
        } else {
            self.message_bar.render(bottom_bar_row);
        }

        if self.terminal_size.height > 1 {
            self.status_bar
                .render(self.terminal_size.height.saturating_sub(2));
        }
        if self.terminal_size.height > 2 {
            self.view.render(0);
        }

        let next = if self.in_prompt() {
            Position {
                y: bottom_bar_row,
                x: self.command_bar.cursor_position_col(),
            }
        } else {
            self.view.caret_position()
        };

        let _ = self.terminal.move_cursor(&next);
        let _ = self.terminal.show_cursor();
        let _ = self.terminal.execute();
    }

    pub fn refresh_status(&mut self) {
        let status = self.view.get_status();
        let title = format!("{} - {NAME}", status.filename);
        self.status_bar.update(status);

        if title != self.title && matches!(self.terminal.set_title(&title), Ok(())) {
            self.title = title;
        }
    }

    fn update_message(&mut self, message: &str) {
        self.message_bar.update(message);
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = self.terminal.terminate();
        if self.should_quit {
            let _ = self.terminal.print("Fin\r\n");
        }
    }
}
