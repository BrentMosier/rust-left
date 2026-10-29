use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use std::{env, fs, io};

struct Editor {
    //each item in the vector is a line of text
    lines: Vec<String>,
    //cursor row and col indices
    cursor_x: usize,
    cursor_y: usize,
    //top and left most character indices currently visible on screen
    row_offset: usize,
    col_offset: usize,
    filename: Option<String>,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_x: 0,
            cursor_y: 0,
            filename: None,
            row_offset: 0,
            col_offset: 0,
        }
    }
}

impl Editor {
    //Create editor instance and load content from file if it exists
    fn open(filename: String) -> Self {
        match fs::read_to_string(&filename) {
            Ok(content) => {
                let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                if lines.is_empty() {
                    lines.push(String::new())
                }
                Self {
                    lines,
                    cursor_x: 0,
                    cursor_y: 0,
                    filename: Some(filename),
                    row_offset: 0,
                    col_offset: 0,
                }
            }
            Err(_) => {
                //if file doesn't exist, treat it as a new file with the specified name
                Self {
                    lines: vec![String::new()],
                    cursor_x: 0,
                    cursor_y: 0,
                    filename: Some(filename),
                    row_offset: 0,
                    col_offset: 0,
                }
            }
        }
    }

    //save the buffer back to disk
    fn save(&self) -> io::Result<()> {
        if let Some(ref path) = self.filename {
            let content = self.lines.join("\n");
            fs::write(path, content)?;
        }
        Ok(())
    }

    fn scroll(&mut self, width: usize, height: usize) {
        //vertical scroll up
        if self.cursor_y < self.row_offset {
            self.row_offset = self.cursor_y;
        }
        //vertical scroll down
        if self.cursor_y >= self.row_offset + height {
            self.row_offset = self.cursor_y - height + 1;
        }
        //horizontal scroll left
        if self.cursor_x < self.col_offset {
            self.col_offset = self.cursor_x;
        }
        //horizontal scroll right
        if self.cursor_x >= self.col_offset + width {
            self.col_offset = self.cursor_x - width + 1;
        }
    }

    fn get_word_count(&self) -> usize {
        self.lines
            .iter()
            .map(|line| line.split_whitespace().count())
            .sum()
    }

    fn insert_char(&mut self, ch: char) {
        if let Some(line) = self.lines.get_mut(self.cursor_y) {
            line.insert(self.cursor_x, ch);
            self.cursor_x += 1;
        }
    }

    fn insert_newline(&mut self) {
        if let Some(line) = self.lines.get_mut(self.cursor_y) {
            //split the line at the current cursor column
            let remaining = line.split_off(self.cursor_x);
            self.lines.insert(self.cursor_y + 1, remaining);
            self.cursor_y += 1;
            self.cursor_x = 0;
        }
    }

    fn delete_char(&mut self) {
        if self.cursor_x > 0 {
            if let Some(line) = self.lines.get_mut(self.cursor_y) {
                line.remove(self.cursor_x - 1);
                self.cursor_x -= 1;
            }
        } else if self.cursor_y > 0 {
            //backspace at the start of a line -> merge with previous line
            let current_line = self.lines.remove(self.cursor_y);
            let prev_line = &mut self.lines[self.cursor_y - 1];
            self.cursor_x = prev_line.len();
            prev_line.push_str(&current_line);
            self.cursor_y -= 1;
        }
    }

    fn move_cursor_left(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = self.lines[self.cursor_y].len();
        }
    }

    fn move_cursor_right(&mut self) {
        if let Some(line) = self.lines.get(self.cursor_y) {
            if self.cursor_x < line.len() {
                self.cursor_x += 1;
            } else if self.cursor_y + 1 < self.lines.len() {
                self.cursor_y += 1;
                self.cursor_x = 0;
            }
        }
    }

    fn move_cursor_up(&mut self) {
        if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = self.cursor_x.min(self.lines[self.cursor_y].len());
        } else {
            //already at top, jump to beginning of line
            self.cursor_x = 0;
        }
    }

    fn move_cursor_down(&mut self) {
        if self.cursor_y + 1 < self.lines.len() {
            self.cursor_y += 1;
            self.cursor_x = self.cursor_x.min(self.lines[self.cursor_y].len());
        } else {
            //already at bottom, jump to end
            self.cursor_x = self.lines[self.cursor_y].len();
        }
    }

    fn lines_get(&self, index: usize) -> Option<&String> {
        self.lines.get(index)
    }
}

fn run_app(terminal: &mut DefaultTerminal, editor: &mut Editor) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, editor))?;

        if let Event::Key(key) = event::read()? {
            //check for exit command
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
                return Ok(());
            }

            //check for save command
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
                let _ = editor.save();
                continue; //stops process so you don't type an S when saving
            }

            match key.code {
                KeyCode::Char(c) => editor.insert_char(c),
                KeyCode::Enter => editor.insert_newline(),
                KeyCode::Backspace => editor.delete_char(),
                KeyCode::Left => editor.move_cursor_left(),
                KeyCode::Right => editor.move_cursor_right(),
                KeyCode::Up => editor.move_cursor_up(),
                KeyCode::Down => editor.move_cursor_down(),
                _ => {}
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, editor: &mut Editor) {
    let size = f.area();

    //split screen: main editing zone (95% height), status bar (1 cell high)
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(size);

    let main_area = chunks[0];
    let status_area = chunks[1];

    // --- render main editor panel ---
    let title = match &editor.filename {
        Some(name) => format!(
            " Rust TUI Text Editor [{}] (CTRL+S: Save | CTRL+Q: Exit) ",
            name
        ),
        None => " Rust TUI Text Editor [No File] (CTRL+Q: Exit) ".to_string(),
    };

    //create an outer frame container
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner_area = block.inner(main_area);
    f.render_widget(block, main_area);

    //update viewport
    let width = inner_area.width as usize;
    let height = inner_area.height as usize;
    editor.scroll(width, height);

    //slice lines array to build the window view representation
    //dynamic markdown syntax highlighter parser loop
    let visible_lines: Vec<Line> = editor
        .lines
        .iter()
        .skip(editor.row_offset)
        .take(height)
        .map(|line| {
            //apply horizontal scroll limits
            let display_text = if line.len() > editor.col_offset {
                let end = (editor.col_offset + width).min(line.len());
                line[editor.col_offset..end].to_string()
            } else {
                String::new()
            };

            //evaluate prefix syntax rules
            if line.starts_with("## ") {
                Line::from(Span::styled(
                    display_text,
                    Style::default().fg(Color::Blue), // .add_modifier(Modifier::BOLD),
                ))
            } else if line.starts_with("# ") {
                Line::from(Span::styled(
                    display_text,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::raw(display_text))
            }
        })
        .collect();

    //convert lines into printable text lines
    let paragraph = Paragraph::new(Text::from(visible_lines));
    f.render_widget(paragraph, inner_area);

    // --- Render status bar panel ---
    let words = editor.get_word_count();
    let total_lines = editor.lines.len();

    //format text layouts for left and right status segments
    let status_left = format!(" W: {} | L: {}", words, total_lines);
    let status_right = format!("Ln {}, Col {} ", editor.cursor_y + 1, editor.cursor_x + 1);

    //padding spaces out the right aligned segment to the terminal edge
    let padding_len =
        (status_area.width as usize).saturating_sub(status_left.len() + status_right.len());
    let padding = " ".repeat(padding_len);

    let theme_text_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let status_text = Line::from(vec![
        Span::styled(status_left, theme_text_style),
        Span::raw(padding),
        Span::styled(status_right, theme_text_style),
    ]);

    //render and inverted bar strip across the bottom layout frame
    let status_bar = Paragraph::new(status_text);
    f.render_widget(status_bar, status_area);

    //explicitly place the terminal blinking cursor at the calculated position
    //accounts for the border padding offset
    //accounts for viewport offsets
    f.set_cursor_position((
        inner_area.x + (editor.cursor_x - editor.col_offset) as u16,
        inner_area.y + (editor.cursor_y - editor.row_offset) as u16,
    ));
}

fn main() -> io::Result<()> {
    //parse command line arguments for a file name
    let args: Vec<String> = env::args().collect();
    let mut editor = if args.len() > 1 {
        Editor::open(args[1].clone())
    } else {
        Editor::default()
    };

    //put terminal into raw mode and get instance
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal, &mut editor);

    //restore normal terminal behavior on exit
    ratatui::restore();
    result
}
