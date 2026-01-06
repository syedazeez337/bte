use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

struct FormApp {
    name: String,
    email: String,
    focused_field: usize,
    errors: Vec<String>,
}

impl FormApp {
    fn new() -> Self {
        Self {
            name: String::new(),
            email: String::new(),
            focused_field: 0,
            errors: Vec::new(),
        }
    }

    fn submit(&mut self) {
        self.errors.clear();
        if self.name.is_empty() {
            self.errors.push("Name is required".to_string());
        }
        if self.email.is_empty() {
            self.errors.push("Email is required".to_string());
        }
        if !self.email.contains('@') {
            self.errors.push("Email must contain @".to_string());
        }
    }
}

fn ui(frame: &mut Frame, app: &FormApp) {
    let chunks = Layout::new(
        Direction::Vertical,
        Constraint::from_lengths([10, 10, 10, 5]),
    )
    .split(frame.area());

    let name_label = match app.focused_field {
        0 => "Name [*]: ",
        _ => "Name: ",
    };
    let name_block = Block::default().title("Contact Form").borders(Borders::ALL);
    let name_input = Paragraph::new(app.name.as_str())
        .style(if app.focused_field == 0 {
            Style::default().fg(ratatui::style::Color::Yellow)
        } else {
            Style::default()
        })
        .block(name_block);
    frame.render_widget(name_input, chunks[0]);

    let email_label = match app.focused_field {
        1 => "Email [*]: ",
        _ => "Email: ",
    };
    let email_block = Block::default().borders(Borders::ALL);
    let email_input = Paragraph::new(app.email.as_str())
        .style(if app.focused_field == 1 {
            Style::default().fg(ratatui::style::Color::Yellow)
        } else {
            Style::default()
        })
        .block(email_block.title(email_label));
    frame.render_widget(email_input, chunks[1]);

    let error_block = Block::default().title("Errors").borders(Borders::ALL);
    let error_text = if app.errors.is_empty() {
        "No errors".to_string()
    } else {
        app.errors.join("\n")
    };
    let errors = Paragraph::new(error_text)
        .style(Style::default().fg(ratatui::style::Color::Red))
        .block(error_block);
    frame.render_widget(errors, chunks[2]);

    let instructions = Paragraph::new("TAB: Next field | ENTER: Submit | q: Quit")
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(instructions, chunks[3]);
}

fn main() {
    let mut terminal = ratatui::init();
    let mut app = FormApp::new();

    loop {
        terminal.draw(|frame| ui(frame, &app)).unwrap();

        if let Ok(event) = ratatui::crossterm::event::read() {
            match event {
                ratatui::crossterm::event::Event::Key(key) => match key.code {
                    ratatui::crossterm::event::KeyCode::Tab => {
                        app.focused_field = (app.focused_field + 1) % 2;
                    }
                    ratatui::crossterm::event::KeyCode::Enter => {
                        app.submit();
                    }
                    ratatui::crossterm::event::KeyCode::Backspace => {
                        let field = if app.focused_field == 0 {
                            &mut app.name
                        } else {
                            &mut app.email
                        };
                        field.pop();
                    }
                    ratatui::crossterm::event::KeyCode::Char(c) => {
                        let field = if app.focused_field == 0 {
                            &mut app.name
                        } else {
                            &mut app.email
                        };
                        field.push(c);
                    }
                    ratatui::crossterm::event::KeyCode::Char('q') => break,
                    _ => {}
                },
                _ => {}
            }
        }
    }

    ratatui::restore();
}
