use std::time::{Duration, Instant};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{
        Block, BorderType, Borders, Cell, Clear, LineGauge, Paragraph, Row, Table, TableState,
        Widget,
    },
};

use crate::{
    helpers::{nice_size, nice_size_g, nice_time},
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
    filter: String,
    filtering: bool,
    hide_cores: bool,
    show_info: Option<usize>,
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
        filter: String::new(),
        filtering: false,
        hide_cores: false,
        show_info: None,
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
            if let Some(pid) = state.show_info
                && let Some(proc) = state.procs.iter().find(|p| p.pid == pid)
            {
                let parent = if let Some(pproc) = state.procs.iter().find(|p| p.pid == proc.parent)
                {
                    pproc.name.clone()
                } else {
                    "".to_string()
                };
                draw_process_info(f, proc, parent);
            }
            draw_filter(f, &state);

            handle_input(&mut done, &mut state);
            state.selected = state
                .selected
                .min(state.visible.procs().len().saturating_sub(1));

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

fn draw_process_info(f: &mut Frame<'_>, proc: &MyProcess, parent: String) {
    let mut lines = vec![
        format!("PID: {}", proc.pid),
        format!("Name: {}", proc.name),
        format!("Command Line: {}", proc.command),
        format!("Memory (self): {}", nice_size(proc.memory)),
        format!("Memory (children): {}", nice_size(proc.children_memory)),
        format!("Memory (total): {}", nice_size(proc.total())),
        format!("CPU: {:>5.1}%", proc.cpu),
        format!("Run Time: {}", nice_time(proc.run_time)),
    ];
    if !parent.is_empty() {
        lines.push(format!("Parent PID: {:?}", proc.parent));
        lines.push(format!("Parent Name: {}", parent));
    }

    let cmd_width = lines[2].len() as u16 + 2;
    let p = Paragraph::new(
        lines
            .iter()
            .map(|a| Line::from(a.clone()))
            .collect::<Vec<Line>>(),
    )
    .style(Style::default().bg(Color::Yellow).fg(Color::Black))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(proc.name.clone())
            .border_type(BorderType::Rounded),
    );
    let w = 40.max(cmd_width).min(f.area().width - 4);
    let h = lines.len() as u16 + 2;
    let rect = f
        .area()
        .centered(Constraint::Length(w), Constraint::Length(h));

    Clear::default().render(rect, f.buffer_mut());
    f.render_widget(p, rect);
}

fn draw_filter(f: &mut Frame, state: &State) {
    if state.filtering || !state.filter.is_empty() {
        let mut style = Style::default();
        if state.filtering {
            style = style.bg(Color::Green).fg(Color::Black);
        }
        let top_height = get_cores_height(state) + 4;
        let p = Paragraph::new(format!("Filter: {}", state.filter)).style(style);
        f.render_widget(p, Rect::new(f.area().width - 40, top_height, 40, 1));
    }
}
fn draw_help(f: &mut Frame) {
    let help = r#"?/F1        Help menu
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
f          Filter processes
c          Hide CPU cores
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
    let x = f.area().width / 3;
    let y = f.area().height.saturating_sub(help.lines().count() as u16) / 3;
    let w = 40.min(f.area().width.saturating_sub(x));
    let h = 20.min(f.area().height.saturating_sub(y));
    let rect = Rect::new(x, y, w, h);
    Clear::default().render(rect, f.buffer_mut());
    f.render_widget(p, rect);
}

fn draw_top(f: &mut Frame, state: &State) {
    let totals = &state.totals;

    let cpu_height = get_cores_height(state);

    //draw cpus
    if !state.hide_cores {
        for (i, cp) in state.totals.cpus.iter().enumerate() {
            let width = f.area().width / 4;
            let x = (i % 4) as u16 * width;
            let y = (i / 4) as u16;
            if y >= f.area().height {
                break;
            }
            draw_cpu(f, x, y, width, *cp / 100., &format!("{}", i + 1));
        }
    }
    draw_cpu(
        f,
        0,
        cpu_height,
        48,
        totals.cpu_avg / 100.,
        &format!("Cpu x{}:", totals.cpu_count),
    );

    draw_mem(totals, f, cpu_height + 1);

    let up = Block::default().title(format!("Uptime: {}", nice_time(state.totals.uptime)));
    f.render_widget(up, Rect::new(0, cpu_height + 2, f.area().width, 1));

    let threads = Block::default().title(format!("Processes: {}", state.procs.len()));
    f.render_widget(threads, Rect::new(0, cpu_height + 3, f.area().width, 1));

    let commands = Block::default()
        .title("?: help  s: Sort Type  c: cpu  enter: info  f: filter ".to_string());
    f.render_widget(commands, Rect::new(0, cpu_height + 4, f.area().width, 1));
}

fn draw_mem(totals: &Totals, f: &mut Frame, y: u16) {
    gauge(
        f,
        0,
        y,
        48,
        totals.memory as f32 / totals.memory_total as f32,
        &format!(
            "Memory: {}/{}",
            nice_size_g(totals.memory),
            nice_size_g(totals.memory_total)
        ),
    )
}

fn gauge(f: &mut Frame, x: u16, y: u16, width: u16, val: f32, title: &str) {
    let col = if val > 70. {
        Color::LightRed
    } else if val > 35. {
        Color::Yellow
    } else {
        Color::White
    };
    let gr = LineGauge::default()
        .label(title)
        .filled_style(Style::default().fg(col).bg(Color::Black))
        .ratio((val as f64) / 100.);

    f.render_widget(gr, Rect::new(x, y, width, 1));
}

fn draw_cpu(f: &mut Frame, x: u16, y: u16, width: u16, cpu: f32, title: &str) {
    gauge(
        f,
        x,
        y,
        width,
        cpu,
        &format!("{title:>6} {:>5.1}%", cpu * 100.),
    );
}

fn draw_table(f: &mut Frame, state: &State, tablestate: &mut TableState) {
    let top_height = get_cores_height(state) + 5;

    let header_cells = Row::new(
        ["Command", "Name", "PID", "Self", "Children", "Total", "CPU"]
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let mut name = h.to_string();
                let mut style = Style::default().fg(Color::Black).bg(Color::LightBlue);

                if i == state.visible.sort_col {
                    style = Style::default().fg(Color::White).bg(Color::LightRed);

                    match state.visible.sort_type {
                        SortType::Ascending => {
                            name.push_str(" ↑");
                        }

                        SortType::Descending => {
                            name.push_str(" ↓");
                        }
                        _ => {}
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
                    2..=6 => format!("{c:>10}"),
                    _ => c.to_string(),
                };
                let pid = f[2].parse::<usize>().unwrap();

                let mut style = if state.top5memory.contains(&pid) && (i == 3 || i < 2) {
                    Style::default().fg(Color::LightRed)
                } else {
                    Style::default()
                };

                if state.top5cpu.contains(&pid) && (i == 6 || i < 2) {
                    style = Style::default().fg(Color::Magenta);
                }

                Cell::from(val).style(style)
            }))
            .height(1)
        })
        .collect();

    let command_width = (f.area().width.max(85) - 85).max(25);
    let widths = [
        Constraint::Min(command_width),
        Constraint::Min(25),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
    ];
    let t = Table::new(rows, widths)
        .header(header_cells)
        .row_highlight_style(Style::default().bg(Color::LightYellow).fg(Color::Black));

    let mut rect = f.area();
    rect.y += top_height;
    rect.height = rect.height.saturating_sub(top_height);
    f.render_stateful_widget(t, rect, tablestate);
}

fn get_cores_height(state: &State) -> u16 {
    if state.hide_cores {
        0
    } else {
        state.totals.cpus.len() as u16 / 4
    }
}

fn handle_input(done: &mut bool, state: &mut State) {
    if event::poll(Duration::from_millis(50)).unwrap()
        && let Ok(Event::Key(key)) = event::read()
        && key.kind == KeyEventKind::Press
    {
        if state.filtering {
            match key.code {
                KeyCode::Esc => {
                    state.filtering = false;
                    state.filter.clear();
                }
                KeyCode::Enter => state.filtering = false,
                KeyCode::Char(c) => state.filter.push(c),
                KeyCode::Backspace => {
                    let _ = state.filter.pop();
                }
                _ => {}
            }
            state.visible.set_filter(state.filter.clone());
        } else if state.help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::F(1) => state.help = false,
                _ => {}
            }
        } else if state.show_info.is_some() {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => state.show_info = None,
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => *done = true,
                KeyCode::Char('s') => {
                    state.visible.sort_cycle();
                    state.sort();
                }
                KeyCode::Char('c') => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        *done = true
                    } else {
                        state.hide_cores = !state.hide_cores;
                    }
                }
                KeyCode::Char('f') => {
                    state.filtering = !state.filtering;
                }
                KeyCode::Char('z') => state.visible.hidezeros = !state.visible.hidezeros,
                KeyCode::Char('?') | KeyCode::F(1) => state.help = !state.help,
                KeyCode::Char('g') => {
                    *done = true;
                    state.start_gui = true
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    state.selected = (state.selected + 1).min(state.visible.procs().len() - 1)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    state.selected = state.selected.saturating_sub(1)
                }
                KeyCode::PageDown => {
                    state.selected = (state.selected + 20).min(state.visible.procs().len() - 1)
                }
                KeyCode::PageUp => state.selected = state.selected.saturating_sub(20),
                KeyCode::Home => state.selected = 0,
                KeyCode::End => state.selected = state.visible.procs().len() - 1,
                KeyCode::Enter => {
                    if state.show_info.is_some() {
                        state.show_info = None;
                        return;
                    }
                    if !state.visible.procs().is_empty()
                        && state.selected < state.visible.procs().len()
                    {
                        let pid = state.visible.procs()[state.selected][2]
                            .parse::<usize>()
                            .unwrap();
                        state.show_info = Some(pid);
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    state.visible.sort_col = state.visible.sort_col.saturating_sub(1);
                    if state.visible.sort_col == 0 {
                        state.visible.sort_type = SortType::None;
                    }
                    state.sort();
                }
                KeyCode::Right | KeyCode::Char('l') => {
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
