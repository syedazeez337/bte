use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

struct LayoutApp {
    selected_row: usize,
    items: Vec<Vec<&'static str>>,
}

impl LayoutApp {
    fn new() -> Self {
        Self {
            selected_row: 0,
            items: vec![
                vec!["Item 1", "Description A"],
                vec!["Item 2", "Description B"],
                vec!["Item 3", "Description C"],
                vec!["Item 4", "Description D"],
                vec!["Item 5", "Description E"],
            ],
        }
    }

    fn move_up(&mut self) {
        if self.selected_row > 0 {
            self.selected_row -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.selected_row < self.items.len() - 1 {
            self.selected_row += 1;
        }
    }
}

fn ui(frame: &mut Frame, app: &LayoutApp) {
    let chunks = Layout::new(
        Direction::Horizontal,
        Constraint::from_percentages([30, 70]),
    )
    .split(frame.area());

    let sidebar = Block::default().title("Sidebar").borders(Borders::ALL);
    let sidebar_content = Paragraph::new("Navigation:\n↑ Up\n↓ Down\nq Quit").block(sidebar);
    frame.render_widget(sidebar_content, chunks[0]);

    let rows: Vec<Row> = app
        .items
        .iter()
        .map(|item| Row::new(item.iter().map(|s| Cell::from(*s)).collect::<Vec<Cell>>()))
        .collect();

    let table = Table::new(rows, Constraint::from_percentages([40, 60]))
        .header(Row::new(vec!["Name", "Description"]).bottom_margin(1))
        .block(Block::default().title("Content List").borders(Borders::ALL))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");

    frame.render_widget(table, chunks[1]);
}

fn main() {
    let mut terminal = ratatui::init();
    let mut app = LayoutApp::new();

    loop {
        terminal.draw(|frame| ui(frame, &app)).unwrap();

        if let Ok(event) = ratatui::crossterm::event::read() {
            match event {
                ratatui::crossterm::event::Event::Key(key) => match key.code {
                    ratatui::crossterm::event::KeyCode::Up => app.move_up(),
                    ratatui::crossterm::event::KeyCode::Down => app.move_down(),
                    ratatui::crossterm::event::KeyCode::Char('q') => break,
                    _ => {}
                },
                _ => {}
            }
        }
    }

    ratatui::restore();
}
