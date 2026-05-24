#![warn(clippy::pedantic, clippy::print_stdout)]
use crate::editor::Editor;

mod annotated;
mod buffer;
mod command;
mod command_bar;
mod document_status;
mod editor;
mod file_info;
mod highlighter;
mod line;
mod message_bar;
mod status_bar;
mod terminal;
mod ui_component;
mod view;

fn main() -> anyhow::Result<()> {
    Editor::new()?.run();
    Ok(())
}
