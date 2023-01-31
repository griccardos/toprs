use std::{
    io::Stdout,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Cell, LineGauge, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};

use crate::{
    helpers::{nice_size_g, nice_time},
    manager::{self, Totals},
    myprocess::MyProcess,
    sorted::{SortType, SortedProcesses},
};

struct State {
    visible: SortedProcesses,
    procs: Vec<MyProcess>,
    totals: Totals,
    selected: usize,
    top5memory: Vec<usize>,
    top5cpu: Vec<usize>,
    help: bool,
    start_gui: bool,
}

pub fn run() -> Result<bool, std::io::Error> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    let mut man = manager::ProcManager::new();
    let mut state = State {
        procs: man.procs().clone(),
        visible: SortedProcesses::new(),
        selected: 0,
        totals: man.get_totals(),
        top5memory: vec![],
        top5cpu: vec![],
        help: false,
        start_gui: false,
    };
    state.sort();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let back = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(back)?;
    let mut done = false;
    let mut last = Instant::now();
    let mut tablestate = TableState::default();

    while !done {
        terminal.draw(|f| {
            //get update if necessary
            if last.elapsed().as_secs_f32() > 2. {
                man.update();
                state.procs = man.procs().clone();
                state.sort();
                state.totals = man.get_totals();
                last = Instant::now();
            }

            draw_top(f, &state);
            draw_table(f, &state, &mut tablestate);

            if state.help {
                draw_help(f);
            }

            handle_input(&mut done, &mut state);

            tablestate.select(Some(state.selected));
        })?;
    }

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    Ok(state.start_gui)
}

fn draw_help(f: &mut Frame<CrosstermBackend<Stdout>>) {
    let help = r#"?/h        Help menu                           
Up         Scroll up                      
Down       Scroll down                       
Left       Sort by column to left            
Right      Sort by column to right            
s          Sort Asc/Desc/None                 
q/Esc      Exit                            
z          Hide/show zero memory            
Home       Go to first row                
End        Go to last row                  
g          Start gui mode                       
                                                       
command line arguments for modes:               
-g         GUI                                           
-t         Terminal mode                               
-s <FILE>  save svg                            
-o         output to stdout                    
                                             "#;

    let p = Paragraph::new(help)
        .style(Style::default().bg(Color::Yellow).fg(Color::Black))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Help")
                .border_type(BorderType::Rounded),
        );
    let x = (f.size().width / 3).max(1);
    let y = (f.size().height / 3).max(1);
    let w = 40.min(f.size().width.saturating_sub(x));
    let h = 20.min(f.size().height.saturating_sub(y));
    f.render_widget(p, Rect::new(x, y, w, h));
}

fn draw_top(f: &mut Frame<CrosstermBackend<Stdout>>, state: &State) {
    let totals = &state.totals;

    let mem = Paragraph::new(format!(
        "Memory: {}/{}",
        nice_size_g(totals.memory),
        nice_size_g(totals.memory_total)
    ));
    f.render_widget(mem, Rect::new(0, 0, f.size().width, 1));

    let gr = LineGauge::default()
        .label(format!("Cpu: {:.2}%", totals.cpu))
        .ratio(totals.cpu as f64 / 100.)
        .line_set(tui::symbols::line::THICK)
        .gauge_style(Style::default().fg(Color::White).bg(Color::LightBlue));

    f.render_widget(gr, Rect::new(0, 1, f.size().width.min(45), 1));

    let up = Block::default().title(format!("Uptime: {}", nice_time(state.totals.uptime)));
    f.render_widget(up, Rect::new(0, 2, f.size().width, 1));
}

fn draw_table(f: &mut Frame<CrosstermBackend<Stdout>>, state: &State, tablestate: &mut TableState) {
    let header_cells = Row::new(
        ["Command", "Name", "PID", "Self", "Children", "Total", "CPU"]
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let mut name = h.to_string();
                let mut style = Style::default().fg(Color::Black).bg(Color::LightBlue);

                if i == state.visible.sort_col {
                    if i == 0 {
                        style = Style::default().fg(Color::White).bg(Color::LightMagenta);
                    } else {
                        match state.visible.sort_type {
                            SortType::Ascending => {
                                style = Style::default().fg(Color::White).bg(Color::LightMagenta);
                                name.push_str(" ↑");
                            }

                            SortType::Descending => {
                                style = Style::default().fg(Color::White).bg(Color::LightMagenta);
                                name.push_str(" ↓");
                            }
                            _ => {}
                        }
                    }
                }
                let name = if (2..=6).contains(&i) {
                    format!("{name:>10}")
                } else {
                    name
                };

                Cell::from(name).style(style)
            }),
    )
    .style(Style::default().bg(Color::LightBlue));

    let rows: Vec<Row> = state
        .visible
        .procs()
        .iter()
        .map(|f| {
            Row::new(f.iter().enumerate().map(|(i, c)| {
                let val = match i {
                    2 | 3 | 4 | 5 | 6 => format!("{c:>10}"),
                    _ => c.to_string(),
                };
                let pid = f[2].parse::<usize>().unwrap();

                let style = if state.top5memory.contains(&pid) && i == 3
                    || state.top5cpu.contains(&pid) && i == 6
                {
                    Style::default().fg(Color::LightRed)
                } else {
                    Style::default()
                };

                let cell = Cell::from(val).style(style);
                //println!("{cell:?}");
                cell
            }))
            .height(1)
        })
        .collect();

    let command_width = (f.size().width.max(85) - 85).max(25);
    let widths = [
        Constraint::Min(command_width),
        Constraint::Min(25),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
    ];
    let t = Table::new(rows)
        .header(header_cells)
        //.block(
        //    Block::default()
        //        .borders(Borders::ALL)
        //        .title("Processes")
        //        .border_type(tui::widgets::BorderType::Rounded),
        //)
        .widths(&widths)
        .highlight_style(Style::default().bg(Color::LightYellow).fg(Color::Black));

    let mut rect = f.size();
    rect.y += 4;
    rect.height -= 4;
    f.render_stateful_widget(t, rect, tablestate);
}

fn handle_input(done: &mut bool, state: &mut State) {
    if event::poll(Duration::from_millis(10)).unwrap() {
        if let Ok(Event::Key(key)) = event::read() {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => *done = true,
                KeyCode::Char('s') => {
                    state.visible.sort_cycle();
                    state.sort();
                }
                KeyCode::Char('c') => {
                    if KeyModifiers::CONTROL.contains(key.modifiers) {
                        *done = true
                    }
                }
                KeyCode::Char('z') => state.visible.hidezeros = !state.visible.hidezeros,
                KeyCode::Char('h') | KeyCode::Char('?') => state.help = !state.help,
                KeyCode::Char('g') => {
                    *done = true;
                    state.start_gui = true
                }
                KeyCode::Down => {
                    state.selected = (state.selected + 1).min(state.visible.procs().len() - 1)
                }
                KeyCode::Up => state.selected = state.selected.saturating_sub(1),
                KeyCode::PageDown => {
                    state.selected = (state.selected + 20).min(state.visible.procs().len() - 1)
                }
                KeyCode::PageUp => state.selected = state.selected.saturating_sub(20),
                KeyCode::Home => state.selected = 0,
                KeyCode::End => state.selected = state.visible.procs().len() - 1,

                KeyCode::Left => {
                    state.visible.sort_col = state.visible.sort_col.saturating_sub(1);
                    if state.visible.sort_col == 0 {
                        state.visible.sort_type = SortType::None;
                    }
                    state.sort();
                }
                KeyCode::Right => {
                    state.visible.sort_col = (state.visible.sort_col + 1).min(6);
                    if state.visible.sort_col > 0 && state.visible.sort_type == SortType::None {
                        state.visible.sort_type = SortType::Descending;
                    }
                    state.sort();
                }
                _ => {}
            }
        }
    }
}

impl State {
    fn sort(&mut self) {
        self.visible.update(&self.procs);

        //find top 5
        let mut temp = self.procs.to_vec();
        temp.sort_by(|a, b| b.memory.cmp(&a.memory));
        self.top5memory = temp.iter().map(|f| f.pid).take(5).collect();

        let mut temp = self.procs.to_vec();
        temp.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap());
        self.top5cpu = temp.iter().map(|f| f.pid).take(5).collect();
    }
}
