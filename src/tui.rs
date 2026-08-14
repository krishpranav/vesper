use crate::core::ScanResult;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::UnboundedReceiver;

pub enum AppEvent {
    Log(String),
    Result(ScanResult),
    Progress { current: usize, total: usize },
    Done,
}

pub struct App {
    pub current_username: String,
    pub found_results: Vec<ScanResult>,
    pub logs: Vec<String>,
    pub progress_current: usize,
    pub progress_total: usize,
    pub start_time: Instant,
    pub is_done: bool,
    pub sort_status: bool,
    pub filter_found: bool,
}

impl App {
    pub fn new(username: String) -> Self {
        Self {
            current_username: username,
            found_results: Vec::new(),
            logs: Vec::new(),
            progress_current: 0,
            progress_total: 0,
            start_time: Instant::now(),
            is_done: false,
            sort_status: false,
            filter_found: true,
        }
    }
}

pub fn run_tui(mut app: App, mut rx: UnboundedReceiver<AppEvent>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, &mut app, &mut rx);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    rx: &mut UnboundedReceiver<AppEvent>,
) -> Result<()> {
    // Set tick rate to 16ms (~60fps) for lower latency event handling and smooth UI updates
    let tick_rate = Duration::from_millis(16);

    loop {
        terminal.draw(|f| ui(f, app))?;

        if crossterm::event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') | KeyCode::Esc = key.code {
                    return Ok(());
                }
                if key.code == KeyCode::Char('c') && key.modifiers.contains(event::KeyModifiers::CONTROL) {
                    return Ok(());
                }
                if key.code == KeyCode::Char('s') {
                    app.sort_status = !app.sort_status;
                }
                if key.code == KeyCode::Char('f') {
                    app.filter_found = !app.filter_found;
                }
            }
        }

        while let Ok(event) = rx.try_recv() {
            match event {
                AppEvent::Log(msg) => {
                    app.logs.push(msg);
                    if app.logs.len() > 100 {
                        app.logs.remove(0);
                    }
                }
                AppEvent::Result(res) => {
                    app.found_results.push(res);
                }
                AppEvent::Progress { current, total } => {
                    app.progress_current = current;
                    app.progress_total = total;
                }
                AppEvent::Done => {
                    app.is_done = true;
                }
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.size());

    let header_text = format!(
        " Vesper OSINT Dashboard | Target: {} | Elapsed: {:.1}s | [s]ort: {} | [f]ilter: {} | (Press 'q' to quit)",
        app.current_username,
        app.start_time.elapsed().as_secs_f32(),
        if app.sort_status { "Status" } else { "Site" },
        if app.filter_found { "Found" } else { "All" }
    );
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(chunks[1]);

    // Table for results
    let header_cells = ["Site", "URL", "Status", "Conf"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let table_header = Row::new(header_cells).style(Style::default().add_modifier(Modifier::BOLD));

    let mut display_results: Vec<&ScanResult> = app.found_results.iter().filter(|res| {
        if app.filter_found {
            res.exist
        } else {
            true
        }
    }).collect();

    if app.sort_status {
        display_results.sort_by(|a, b| {
            let a_score = match a.status.as_tag() {
                "CONFIRMED" => 0,
                "LIKELY" => 1,
                "PRIVATE" => 2,
                _ => 3,
            };
            let b_score = match b.status.as_tag() {
                "CONFIRMED" => 0,
                "LIKELY" => 1,
                "PRIVATE" => 2,
                _ => 3,
            };
            a_score.cmp(&b_score).then(a.site.cmp(&b.site))
        });
    } else {
        display_results.sort_by(|a, b| a.site.cmp(&b.site));
    }

    let rows = display_results.into_iter().map(|res| {
        let conf_str = if res.confidence > 0.0 {
            format!("{:.0}%", res.confidence * 100.0)
        } else {
            String::from("-")
        };
        
        let status_color = match res.status.as_tag() {
            "CONFIRMED" | "LIKELY" => Color::Green,
            "NOT_FOUND" => Color::Red,
            "PRIVATE" => Color::Yellow,
            "BLOCKED" | "SOFT_404" | "REDIRECTED" | "ERROR" => Color::Gray,
            _ => Color::White,
        };

        Row::new(vec![
            Cell::from(res.site.clone()),
            Cell::from(res.link.clone()),
            Cell::from(res.status.as_tag()).style(Style::default().fg(status_color)),
            Cell::from(conf_str),
        ])
    });

    let table = Table::new(rows, [
        Constraint::Percentage(20),
        Constraint::Percentage(60),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
    ])
    .header(table_header)
    .block(Block::default().borders(Borders::ALL).title("Found Profiles"));

    f.render_widget(table, middle_chunks[0]);

    // Logs
    let log_text = app.logs.join("\n");
    let logs = Paragraph::new(log_text)
        .block(Block::default().borders(Borders::ALL).title("Event Log"));
    f.render_widget(logs, middle_chunks[1]);

    // Progress
    let progress_ratio = if app.progress_total > 0 {
        app.progress_current as f64 / app.progress_total as f64
    } else {
        0.0
    };
    
    let status_str = if app.is_done {
        "✓ COMPLETED".to_string()
    } else {
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            [(app.start_time.elapsed().as_millis() / 80) as usize % 10];
        format!("{} SCANNING", spinner)
    };

    let found_count = app.found_results.iter().filter(|r| r.exist).count();
    let progress_text = format!(
        " [{}] {} / {} sites checked | Found: {} ",
        status_str,
        app.progress_current,
        app.progress_total,
        found_count
    );
    
    let progress_width = chunks[2].width as usize - progress_text.len() - 4;
    let filled = (progress_ratio * progress_width as f64) as usize;
    let empty = progress_width.saturating_sub(filled);
    
    let bar = format!(" [{}{}]", "█".repeat(filled), "░".repeat(empty));
    
    let footer_text = format!("{}{}", progress_text, bar);

    let footer = Paragraph::new(footer_text)
        .style(if app.is_done { Style::default().fg(Color::Green) } else { Style::default().fg(Color::White) })
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}
