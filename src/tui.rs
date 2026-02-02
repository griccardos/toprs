use ratatui::{style::Color, widgets::Widget};

struct State {
    visible: SortedProcesses,
    procs: Vec<MyProcess>,
    totals: Totals,
    selected: usize,
    kill_signal: usize,
    kill_process: Option<MyProcess>,
    top5memory: Vec<usize>,
    top5cpu: Vec<usize>,
    start_gui: bool,
    filter: String,
    filtering: bool,
    hide_cores: bool,
    show_info: Option<usize>,
    show_kill: bool,
    show_help: bool,
    config: Config,
    searching: bool, //change selection to match
    search: String,  //for changing selection search
}

pub fn run(config: Config) -> Result<bool, std::io::Error> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    let mut man = manager::ProcManager::new();
    let mut state = State {
        procs: man.procs().clone(),
        visible: SortedProcesses::new(),
        selected: 0,
        kill_signal: 9,
        kill_process: None,
        totals: man.get_totals(),
        top5memory: vec![],
        top5cpu: vec![],
        start_gui: false,
        filter: String::new(),
        filtering: false,
        hide_cores: false,
        show_info: None,
        show_kill: false,
        show_help: false,
        config,
        searching: false,
        search: String::new(),
    };
    state.visible.sort_col = state.config.tui.sort_column;
    state.visible.sort_type = state.config.tui.sort_type;
    state.hide_cores = !state.config.tui.show_cpu_per_core;
    state.sort();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let back = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(back)?;
    let mut done = false;
    let mut last = Instant::now();
    let mut tablestate = TableState::default();
    let mut tablestate_kill = TableState::default();

    while !done {
        terminal.draw(|f| {
            //get update if necessary
            if last.elapsed().as_secs_f32() > state.config.tui.update_interval {
                man.update();
                state.procs = man.procs().clone();
                state.sort();
                state.totals = man.get_totals();
                last = Instant::now();
            }

            draw_top(f, &state);
            draw_table(f, &state, &mut tablestate);

            if state.show_help {
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
            if state.show_kill {
                draw_kill(f, &mut tablestate_kill, &state);
            }

            if state.searching {
                draw_search(f, &state);
            } else if state.filtering || !state.filter.is_empty() {
                draw_filter(f, &state);
            }

            handle_input(&mut done, &mut state);

            //update selected for table
            state.selected = state
                .selected
                .min(state.visible.procs().len().saturating_sub(1));
            tablestate.select(Some(state.selected));

            state.kill_signal = state.kill_signal.min(20);
            tablestate_kill.select(Some(state.kill_signal));
        })?;
    }

    //save config
    state.config.tui.sort_column = state.visible.sort_col;
    state.config.tui.sort_type = state.visible.sort_type;
    state.config.tui.show_cpu_per_core = !state.hide_cores;
    state.config.save();

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    Ok(state.start_gui)
}

fn draw_kill(f: &mut Frame<'_>, tablestate: &mut TableState, state: &State) {
    let Some(proc) = &state.kill_process else {
        return;
    };

    let rows = vec![
        "0: Cancel",
        "1: SIGHUP - Hangup",
        "2: SIGINT - Interrupt",
        "3: SIGQUIT - Quit",
        "4: SIGILL - Illegal Instruction",
        "5: SIGTRAP - Trace/Breakpoint Trap",
        "6: SIGABRT - Abort",
        "7: SIGBUS - Bus Error",
        "8: SIGFPE - Floating Point Exception",
        "9: SIGKILL - Kill",
        "10: SIGUSR1 - User Defined Signal 1",
        "11: SIGSEGV - Segmentation Fault",
        "12: SIGUSR2 - User Defined Signal 2",
        "13: SIGPIPE - Broken Pipe",
        "14: SIGALRM - Alarm Clock",
        "15: SIGTERM - Terminate",
        "16: SIGSTKFLT - Stack Fault",
        "17: SIGCHLD - Child Status Has Changed",
        "18: SIGCONT - Continue",
        "19: SIGSTOP - Stop",
        "20: SIGTSTP - Terminal Stop",
    ];
    let max_wid = rows.iter().map(|a| a.len()).max().unwrap_or(10) as u16;
    let widths = [Constraint::Length(max_wid)];

    let t = Table::new(
        rows.iter().map(|a| Row::new(vec![*a])).collect::<Vec<_>>(),
        widths,
    )
    .row_highlight_style(Style::default().bg(Color::LightYellow).fg(Color::Black))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(2))
            .border_style(Style::default().fg(Color::Red))
            .title(format!("Send signal to {} ({})", proc.name, proc.pid))
            .border_type(BorderType::Rounded),
    );

    let rect = f
        .area()
        .centered(Constraint::Length(max_wid + 6), Constraint::Length(23));

    f.render_widget(Clear, rect);
    f.render_stateful_widget(t, rect, tablestate);
}

fn draw_process_info(f: &mut Frame<'_>, proc: &MyProcess, parent: String) {
    let mut lines = vec![
        format!("PID: {}", proc.pid),
        format!("Name: {}", proc.name),
        format!("Command Line: {}", proc.command),
        format!("Memory (self):     {:>10}", nice_size(proc.memory)),
        format!("Memory (children): {:>10}", nice_size(proc.children_memory)),
        format!("Memory (total):    {:>10}", nice_size(proc.total())),
        format!("CPU: {:>5.1}%", proc.cpu),
        format!("Run Time: {}", nice_time(proc.run_time)),
    ];
    if !parent.is_empty() {
        lines.insert(3, format!("Parent PID: {:?}", proc.parent));
        lines.insert(4, format!("Parent Name: {}", parent));
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

    f.render_widget(Clear, rect);
    f.render_widget(p, rect);
}

fn draw_filter(f: &mut Frame, state: &State) {
    let mut style = Style::default();
    if state.filtering {
        style = style.bg(Color::Green).fg(Color::Black);
    } else {
        style = style.fg(Color::Green);
    }
    let top_height = get_cores_height(state) + 4;
    let p = Paragraph::new(format!("Filter: {}", state.filter)).style(style);
    f.render_widget(p, Rect::new(f.area().width - 40, top_height, 40, 1));
}
fn draw_search(f: &mut Frame, state: &State) {
    let mut style = Style::default();
    if state.searching {
        style = style.bg(Color::Green).fg(Color::Black);
    }
    let top_height = get_cores_height(state) + 4;
    let p = Paragraph::new(format!("Search: {}", state.search)).style(style);
    f.render_widget(p, Rect::new(f.area().width - 40, top_height, 40, 1));
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
g          Start GUI mode
f          Filter processes
c          Hide CPU cores
+/-        Increase/decrease interval
command line arguments for modes:
-g         Graphical mode
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
    f.render_widget(Clear, rect);
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
            draw_sub_cpu(
                f,
                Rect::new(x, y, width, 1),
                *cp / 100.,
                &format!("{}", i + 1),
            );
        }
    }
    let max_cpu = totals.cpus.iter().cloned().fold(0., f32::max) / 100.;
    draw_cpu_summary(
        f,
        Rect::new(0, cpu_height, 48, 1),
        totals.cpu_avg / 100.,
        max_cpu,
        totals.cpus.iter().sum::<f32>() / 100.,
        &format!("Cpu x{}:", totals.cpu_count),
    );

    draw_mem(totals, f, cpu_height + 1);

    let up = Block::default().title(format!("Uptime: {}", nice_time(state.totals.uptime)));
    f.render_widget(up, Rect::new(0, cpu_height + 2, f.area().width, 1));

    let threads = Block::default().title(format!(
        "Processes: {}   Interval: {}s",
        state.procs.len(),
        state.config.tui.update_interval
    ));
    f.render_widget(threads, Rect::new(0, cpu_height + 3, f.area().width, 1));

    let commands = Block::default().title(
        "?: help  s: Sort Type  c: CPU  enter: info  f: filter  /:search  ctrl+k: kill "
            .to_string(),
    );
    f.render_widget(commands, Rect::new(0, cpu_height + 4, f.area().width, 1));
}

fn draw_mem(totals: &Totals, f: &mut Frame, y: u16) {
    let col = get_gradient(totals.memory as f32 / totals.memory_total as f32);
    let line = Line::from(vec![
        Span::raw("Memory: "),
        Span::raw(format!(
            "{}/{}",
            nice_size_g(totals.memory),
            nice_size_g(totals.memory_total)
        )),
        Span::styled(
            format!(
                "{:>5.1}%",
                totals.memory as f32 / totals.memory_total as f32 * 100.
            ),
            Style::default().fg(col),
        ),
    ]);
    let len = line.iter().map(|s| s.content.len() as u16).sum::<u16>();
    let mut rect = Rect::new(0, y, 48, 1);
    line.render(rect, f.buffer_mut());
    rect.x += len + 1;
    rect.width = 25;

    gauge(
        f,
        rect,
        totals.memory as f32 / totals.memory_total as f32,
        "",
    )
}

fn gauge<T>(f: &mut Frame, rect: Rect, percentage: f32, title: T)
where
    T: Into<Line<'static>>,
{
    //go from yellow to red depending on value by exponential gradient
    let col = get_gradient(percentage);

    let gr = LineGauge::default()
        .label(title)
        .filled_symbol("■")
        .unfilled_symbol("━")
        .filled_style(Style::new().fg(col))
        .unfilled_style(Style::new().fg(Color::DarkGray))
        .ratio(percentage as f64);

    f.render_widget(gr, rect);
}

fn get_gradient(val: f32) -> Color {
    let red = 255;
    let green = (255. * (1. - val.clamp(0., 1.).powf(2.0))) as u8;

    Color::Rgb(red, green, 0)
}

fn draw_sub_cpu(f: &mut Frame, rect: Rect, cpu: f32, title: &str) {
    gauge(f, rect, cpu, format!("{title:>6} {:>5.1}%", cpu * 100.));
}
fn draw_cpu_summary(f: &mut Frame, rect: Rect, cpu: f32, max_cpu: f32, sum_cpu: f32, title: &str) {
    let mut details = vec![Span::raw(format!("{title:>6} "))];
    //only add for summary cpu:

    let col_avg = get_gradient(cpu);
    details.push(Span::raw(" Avg:"));
    details.push(Span::styled(
        format!("{:>5.1}%", cpu * 100.),
        Style::default().fg(col_avg),
    ));

    details.push(Span::raw(" Max:".to_string()));
    let col_max = get_gradient(max_cpu);
    details.push(Span::styled(
        format!("{:>5.1}%", max_cpu * 100.),
        Style::default().fg(col_max),
    ));
    let col_sum = get_gradient(sum_cpu);
    details.push(Span::raw(" Sum:"));
    details.push(Span::styled(
        format!("{:>6.1}%", sum_cpu * 100.),
        Style::default().fg(col_sum),
    ));
    let len = details.iter().map(|s| s.content.len() as u16).sum::<u16>();
    let line = Line::from(details);
    f.render_widget(line, rect);
    let mut rect = rect;
    rect.x += len + 1;
    rect.width = 25;
    gauge(f, rect, cpu, "");
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
        } else if state.searching {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    state.searching = false;
                    state.search.clear();
                }
                KeyCode::Char(c) => {
                    state.search.push(c);
                    state.update_search();
                }
                KeyCode::Backspace => {
                    let _ = state.search.pop();
                    state.update_search();
                }
                _ => {}
            }
            state.visible.set_filter(state.filter.clone());
        } else if state.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::F(1) => state.show_help = false,
                _ => {}
            }
        } else if state.show_info.is_some() {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => state.show_info = None,
                _ => {}
            }
        } else if state.show_kill {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    state.kill_signal = (state.kill_signal + 1).min(20)
                }
                KeyCode::Char(c) if c >= '0' && c <= '9' => {
                    state.kill_signal = c as usize - '0' as usize;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    state.kill_signal = state.kill_signal.saturating_sub(1)
                }
                KeyCode::Esc => state.show_kill = false,
                KeyCode::Enter => {
                    if state.kill_signal == 0 {
                        //cancel
                    } else {
                        //send signal
                        let signal = state.kill_signal as i32;
                        if let Some(proc) = &state.kill_process {
                            let _ = Command::new("kill")
                                .arg(format!("-{signal}"))
                                .arg(format!("{}", proc.pid))
                                .output();
                        }
                    }
                    state.show_kill = false;
                }
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
                KeyCode::Char('/') => {
                    state.searching = true;
                }
                KeyCode::Char('z') => state.visible.hidezeros = !state.visible.hidezeros,
                KeyCode::Char('?') | KeyCode::F(1) => state.show_help = !state.show_help,
                KeyCode::Char('g') => {
                    *done = true;
                    state.start_gui = true
                }
                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.show_kill = !state.show_kill;
                    if let Some(pid) = state
                        .visible
                        .procs()
                        .get(state.selected)
                        .and_then(|a| a.get(2))
                    {
                        state.kill_process = state
                            .procs
                            .iter()
                            .find(|a| a.pid.to_string() == *pid)
                            .cloned();
                    }
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
                KeyCode::Char('-') => {
                    state.config.tui.update_interval =
                        (state.config.tui.update_interval - 0.5).max(0.5);
                }
                KeyCode::Char('+') => {
                    state.config.tui.update_interval += 0.5;
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

    fn update_search(&mut self) {
        //we find the first process matching the search string
        if let Some(pos) = self.visible.procs().iter().position(|a| {
            a.iter()
                .any(|a| a.to_lowercase().contains(&self.search.to_lowercase()))
        }) {
            self.selected = pos;
        }
    }
}

use std::{
    process::Command,
    time::{Duration, Instant},
};

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
    style::Style,
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, LineGauge, Padding, Paragraph, Row, Table,
        TableState,
    },
};

use crate::{
    config::Config,
    helpers::{nice_size, nice_size_g, nice_time},
    manager::{self, Totals},
    myprocess::MyProcess,
    sorted::{SortType, SortedProcesses},
};
