use std::{error::Error, io};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Spans,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};

struct Item {
    title: String,
    extended: String,
    expanded: bool,
}

struct PlanDisplay {
    info_titles: Vec<String>,
    items: Vec<Item>,
    list_state: ListState,
    input: String,
    input_mode: bool,
}

impl PlanDisplay {
    fn new() -> PlanDisplay {
        let info_titles = vec![
            "Placeholder A".into(),
            "Placeholder B".into(),
            "Placeholder C".into(),
        ];
        let items = (1..=1000)
            .map(|i| Item {
                title: format!("Item {}", i),
                extended: format!("This is the extended text for item {}.\nIt can be multiple lines.", i),
                expanded: false,
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(0));

        PlanDisplay {
            info_titles,
            items,
            list_state,
            input: String::new(),
            input_mode: false,
        }
    }

    fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) if i + 1 < self.items.len() => i + 1,
            _ => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(0) | None => self.items.len() - 1,
            Some(i) => i - 1,
        };
        self.list_state.select(Some(i));
    }

    fn toggle_expand(&mut self) {
        if let Some(i) = self.list_state.selected() {
            self.items[i].expanded = !self.items[i].expanded;
        }
    }
}

pub fn main() -> Result<(), Box<dyn Error>> {
    // --- Terminal setup ---
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // --- Run the app ---
    let mut app = PlanDisplay::new();
    let res = run_app(&mut terminal, &mut app);

    // --- Restore terminal ---
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut PlanDisplay) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        // handle input
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => return Ok(()),      // quit
                KeyCode::Tab => {
                    // switch focus between list and input
                    app.input_mode = !app.input_mode;
                }
                kc => {
                    if app.input_mode {
                        // only accept digits & backspace in input
                        match kc {
                            KeyCode::Char(c) if c.is_ascii_digit() => {
                                app.input.push(c);
                            }
                            KeyCode::Backspace => {
                                app.input.pop();
                            }
                            _ => {}
                        }
                    } else {
                        // list navigation & expand/collapse
                        match kc {
                            KeyCode::Down => app.next(),
                            KeyCode::Up => app.previous(),
                            KeyCode::Enter => app.toggle_expand(),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut PlanDisplay) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(0)].as_ref())
        .split(f.size());

    // ┌── Sidebar ──┐
    // │ Placeholder │
    // │ Placeholder │
    // │ Placeholder │
    // └─────────────┘
    let sidebar_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            app.info_titles
                .iter()
                .map(|_| Constraint::Length(5))
                .collect::<Vec<Constraint>>()
        )
        .split(chunks[0]);
    for (i, title) in app.info_titles.iter().enumerate() {
        let block = Block::default().borders(Borders::ALL).title(title.as_str());
        f.render_widget(block, sidebar_chunks[i]);
    }

    // ┌── Main region ──┐
    // │ [scrollable]    │
    // │ list of widgets │
    // │                ↓│
    // ├─────────────────┤
    // │ Type 4925 ...   │
    // │ ┌─────────────┐ │
    // │ │   4925      │ │
    // │ └─────────────┘ │
    // └─────────────────┘
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(chunks[1]);

    // build list items, injecting extended text when expanded
    let items: Vec<ListItem> = app
        .items
        .iter()
        .map(|it| {
            if it.expanded {
                ListItem::new(vec![
                    Spans::from(it.title.clone()),
                    Spans::from(it.extended.clone()),
                ])
            } else {
                ListItem::new(Spans::from(it.title.clone()))
            }
        })
        .collect();

    let mut state = app.list_state.clone();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Items"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, right_chunks[0], &mut state);
    app.list_state = state;

    // bottom instruction + input field
    let bottom = right_chunks[1];
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(20)].as_ref())
        .split(bottom);

    let instruction = Paragraph::new("Type 4925 to apply all of these operations")
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(instruction, bottom_chunks[0]);

    let input = Paragraph::new(app.input.as_ref())
        .style(
            if app.input_mode {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            },
        )
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, bottom_chunks[1]);

    // if we're in input mode, show the cursor in the input box
    if app.input_mode {
        f.set_cursor(
            // cursor x = input box left + text length + 1 for border
            bottom_chunks[1].x + app.input.len() as u16 + 1,
            bottom_chunks[1].y + 1, // one line down inside the box
        );
    }
}
