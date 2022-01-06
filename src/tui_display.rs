use std::io;
use std::sync::mpsc;
use std::thread;

use log::*;

use termion::{
    input::TermRead,
    raw::{IntoRawMode, RawTerminal},
    screen::AlternateScreen,
};

use tui::backend::Backend;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, Gauge, Tabs};
use tui::Frame;
use tui::Terminal;
use tui_logger::*;

use crate::error::*;
use crate::event;

pub struct TuiApp {
    terminal: Option<Terminal<TermionBackend<AlternateScreen<RawTerminal<io::Stdout>>>>>,
    states: Vec<TuiWidgetState>,
    tabs: Vec<String>,
    selected_tab: usize,
    opt_info_cnt: Option<u16>,
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
            opt_info_cnt: None,
        }
    }
    pub fn init(tx: mpsc::Sender<event::Event>) -> Self {
        let backend = {
            let stdout = io::stdout().into_raw_mode().unwrap();
            let stdout = AlternateScreen::from(stdout);
            TermionBackend::new(stdout)
        };

        let mut terminal = Terminal::new(backend).unwrap();
        terminal.clear().unwrap();
        terminal.hide_cursor().unwrap();

        thread::spawn({
            let stdin = io::stdin();
            move || {
                for c in stdin.events() {
                    trace!(target:"DEMO", "Stdin event received {:?}", c);
                    use termion::event::Key;
                    match c.unwrap() {
                        termion::event::Event::Key(Key::Char('q')) => {
                            tx.send(event::Event::CtrlC).unwrap();
                            break;
                        }
                        termion::event::Event::Key(key) => {
                            use crate::event::Event::*;
                            use TuiAppEvent::*;
                            match key {
                                Key::Char(' ') => {
                                    tx.send(TuiApp(SpaceKey)).unwrap();
                                }
                                Key::Esc => {
                                    tx.send(TuiApp(EscapeKey)).unwrap();
                                }
                                Key::PageUp => {
                                    tx.send(TuiApp(PrevPageKey)).unwrap();
                                }
                                Key::PageDown => {
                                    tx.send(TuiApp(NextPageKey)).unwrap();
                                }
                                Key::Up => {
                                    tx.send(TuiApp(UpKey)).unwrap();
                                }
                                Key::Down => {
                                    tx.send(TuiApp(DownKey)).unwrap();
                                }
                                Key::Left => {
                                    tx.send(TuiApp(LeftKey)).unwrap();
                                }
                                Key::Right => {
                                    tx.send(TuiApp(RightKey)).unwrap();
                                }
                                Key::Char('+') => {
                                    tx.send(TuiApp(PlusKey)).unwrap();
                                }
                                Key::Char('-') => {
                                    tx.send(TuiApp(MinusKey)).unwrap();
                                }
                                Key::Char('h') => {
                                    tx.send(TuiApp(HideKey)).unwrap();
                                }
                                Key::Char('f') => {
                                    tx.send(TuiApp(FocusKey)).unwrap();
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        TuiApp {
            terminal: Some(terminal),
            states: vec![],
            tabs: vec!["V1".into()],
            selected_tab: 0,
            opt_info_cnt: None,
        }
    }
    pub fn deinit(&mut self) {
        if let Some(terminal) = self.terminal.as_mut() {
            terminal.show_cursor().unwrap();
            terminal.clear().unwrap();
        }
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
        app.states.push(TuiWidgetState::new());
    }

    let block = Block::default().borders(Borders::ALL);
    let inner_area = block.inner(size);
    t.render_widget(block, size);

    let mut constraints = vec![
        Constraint::Length(3),
        Constraint::Percentage(50),
        Constraint::Min(3),
    ];
    if app.opt_info_cnt.is_some() {
        constraints.push(Constraint::Length(3));
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner_area);

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
    let tui_w: TuiLoggerWidget = TuiLoggerWidget::default()
        .block(
            Block::default()
                .title("Independent Tui Logger View")
                .border_style(Style::default().fg(Color::White).bg(Color::Black))
                .borders(Borders::ALL),
        )
        .output_separator('|')
        .output_timestamp(Some("%F %H:%M:%S%.3f".to_string()))
        .output_level(Some(TuiLoggerLevelOutput::Long))
        .output_target(false)
        .output_file(false)
        .output_line(false)
        .style(Style::default().fg(Color::White).bg(Color::Black));
    t.render_widget(tui_w, chunks[2]);
    if let Some(percent) = app.opt_info_cnt {
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Progress"))
            .gauge_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::ITALIC),
            )
            .percent(percent);
        t.render_widget(gauge, chunks[3]);
    }
}