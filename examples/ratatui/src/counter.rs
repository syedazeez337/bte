use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

struct App {
    counter: u32,
    name: String,
}

impl App {
    fn new() -> Self {
        Self {
            counter: 0,
            name: "World".to_string(),
        }
    }

    fn increment(&mut self) {
        self.counter += 1;
    }

    fn decrement(&mut self) {
        if self.counter > 0 {
            self.counter -= 1;
        }
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let vertical =
        Layout::new(Direction::Vertical, Constraint::from_percentages([50, 50])).split(area);

    let counter_block = Block::default().title("Counter").borders(Borders::ALL);

    let counter_text = Paragraph::new(format!("Counter: {}", app.counter)).block(counter_block);

    frame.render_widget(counter_text, vertical[0]);

    let instructions = Paragraph::new("Press 'i' to increment, 'd' to decrement, 'q' to quit")
        .block(Block::default().title("Instructions").borders(Borders::ALL));

    frame.render_widget(instructions, vertical[1]);
}

fn main() {
    let mut terminal = ratatui::init();
    let mut app = App::new();

    loop {
        terminal.draw(|frame| ui(frame, &app)).unwrap();

        if let Ok(event) = ratatui::crossterm::event::read() {
            match event {
                ratatui::crossterm::event::Event::Key(key) => match key.code {
                    ratatui::crossterm::event::KeyCode::Char('i') => app.increment(),
                    ratatui::crossterm::event::KeyCode::Char('d') => app.decrement(),
                    ratatui::crossterm::event::KeyCode::Char('q') => break,
                    _ => {}
                },
                _ => {}
            }
        }
    }

    ratatui::restore();
}
