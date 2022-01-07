use std::io;
use std::sync::mpsc;
use std::thread;

use log::*;

use crossterm::event::{read, Event, KeyCode};
//use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use tui::backend::Backend;
use tui::backend::CrosstermBackend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, Tabs};
use tui::Frame;
use tui::Terminal;
use tui_logger::*;

use crate::error::*;
use crate::event;

pub struct TuiApp {
    terminal: Option<Terminal<CrosstermBackend<io::Stdout>>>,
    states: Vec<TuiWidgetState>,
    tabs: Vec<String>,
    selected_tab: usize,
}

#[derive(Debug)]
pub enum TuiAppEvent {
    SpaceKey,
    EscapeKey,
    PrevPageKey,
    NextPageKey,
    UpKey,
    DownKey,
    LeftKey,
    RightKey,
    PlusKey,
    MinusKey,
    HideKey,
    FocusKey,
    TabKey,
}

impl TuiApp {
    pub fn off() -> Self {
        TuiApp {
            terminal: None,
            states: vec![],
            tabs: vec![],
            selected_tab: 0,
        }
    }
    pub fn init(tx: mpsc::Sender<event::Event>) -> BoxResult<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            //EnableMouseCapture
        )?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        terminal.clear().unwrap();
        terminal.hide_cursor().unwrap();

        thread::spawn({
            move || loop {
                let evt = read();
                trace!("Event received {:?}", evt);
                if let Ok(Event::Key(keyevent)) = evt {
                    use crate::event::Event::*;
                    use TuiAppEvent::*;
                    match keyevent.code {
                        KeyCode::Char('q') => {
                            tx.send(event::Event::CtrlC).unwrap();
                            break;
                        }
                        KeyCode::Char(' ') => {
                            tx.send(TuiApp(SpaceKey)).unwrap();
                        }
                        KeyCode::Esc => {
                            tx.send(TuiApp(EscapeKey)).unwrap();
                        }
                        KeyCode::PageUp => {
                            tx.send(TuiApp(PrevPageKey)).unwrap();
                        }
                        KeyCode::PageDown => {
                            tx.send(TuiApp(NextPageKey)).unwrap();
                        }
                        KeyCode::Up => {
                            tx.send(TuiApp(UpKey)).unwrap();
                        }
                        KeyCode::Down => {
                            tx.send(TuiApp(DownKey)).unwrap();
                        }
                        KeyCode::Left => {
                            tx.send(TuiApp(LeftKey)).unwrap();
                        }
                        KeyCode::Right => {
                            tx.send(TuiApp(RightKey)).unwrap();
                        }
                        KeyCode::Char('+') => {
                            tx.send(TuiApp(PlusKey)).unwrap();
                        }
                        KeyCode::Char('-') => {
                            tx.send(TuiApp(MinusKey)).unwrap();
                        }
                        KeyCode::Char('h') => {
                            tx.send(TuiApp(HideKey)).unwrap();
                        }
                        KeyCode::Char('f') => {
                            tx.send(TuiApp(FocusKey)).unwrap();
                        }
                        _ => {}
                    }
                }
            }
        });

        Ok(TuiApp {
            terminal: Some(terminal),
            states: vec![],
            tabs: vec!["1","2","3","4","5","6","7","8","9"].into_iter().map(|t| t.into()).collect(),
            selected_tab: 0,
        })
    }
    pub fn deinit(&mut self) -> BoxResult<()> {
        if let Some(terminal) = self.terminal.as_mut() {
            // restore terminal
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                //DisableMouseCapture
            )?;
            terminal.show_cursor()?;
        }
        Ok(())
    }
    pub fn process_event(&mut self, evt: TuiAppEvent) {
        use TuiAppEvent::*;
        let widget_evt: Option<TuiWidgetEvent> = match evt {
            SpaceKey => Some(TuiWidgetEvent::SpaceKey),
            EscapeKey => Some(TuiWidgetEvent::EscapeKey),
            PrevPageKey => Some(TuiWidgetEvent::PrevPageKey),
            NextPageKey => Some(TuiWidgetEvent::NextPageKey),
            UpKey => Some(TuiWidgetEvent::UpKey),
            DownKey => Some(TuiWidgetEvent::DownKey),
            LeftKey => Some(TuiWidgetEvent::LeftKey),
            RightKey => Some(TuiWidgetEvent::RightKey),
            PlusKey => Some(TuiWidgetEvent::PlusKey),
            MinusKey => Some(TuiWidgetEvent::MinusKey),
            HideKey => Some(TuiWidgetEvent::HideKey),
            FocusKey => Some(TuiWidgetEvent::FocusKey),
            TabKey => None,
        };
        if let Some(widget_evt) = widget_evt {
            self.states[self.selected_tab].transition(&widget_evt);
        }
    }
    pub fn draw(&mut self) -> BoxResult<()> {
        if let Some(mut terminal) = self.terminal.take() {
            terminal.draw(|f| {
                let size = f.size();
                draw_frame(f, size, self);
            })?;
            self.terminal = Some(terminal);
        }
        Ok(())
    }
}
fn draw_frame<B: Backend>(t: &mut Frame<B>, size: Rect, app: &mut TuiApp) {
    let tabs: Vec<Spans> = app
        .tabs
        .iter()
        .map(|t| Spans::from(vec![Span::raw(t)]))
        .collect();
    let sel = app.selected_tab;

    if app.states.len() <= sel {
        app.states.push(TuiWidgetState::new().set_default_display_level(LevelFilter::Info));
    }

    let constraints = vec![
        Constraint::Length(3),
        Constraint::Min(3),
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(size);

    let tabs = Tabs::new(tabs)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .select(sel);
    t.render_widget(tabs, chunks[0]);

    let tui_sm = TuiLoggerSmartWidget::default()
        .style_error(Style::default().fg(Color::Red))
        .style_debug(Style::default().fg(Color::Green))
        .style_warn(Style::default().fg(Color::Yellow))
        .style_trace(Style::default().fg(Color::Magenta))
        .style_info(Style::default().fg(Color::Cyan))
        .output_separator(':')
        .output_timestamp(Some("%H:%M:%S".to_string()))
        .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
        .output_target(true)
        .output_file(true)
        .output_line(true)
        .state(&app.states[sel]);
    t.render_widget(tui_sm, chunks[1]);
}
