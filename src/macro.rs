use macroquad::{
    prelude::{mouse_position, Color},
    shapes::{draw_rectangle, draw_rectangle_lines},
    text::{draw_text, draw_text_ex, load_ttf_font_from_bytes, Font, TextParams},
    window::{next_frame, screen_height, screen_width},
};

#[macroquad::main("memgraph")]
async fn main() {
    let ops = Args::parse_args_default_or_exit();
    let update_rate = 1.;
    let mut sys = sysinfo::System::new_all();
    let mut procs = update_procs(&mut sys, StatKind::Memory);
    //let mut procs = from_file();

    let mut time = Instant::now();
    procs.sort_by(|a, b| b.total().cmp(&a.total()));

    if let Some(path) = ops.svg {
        draw_flamegraph(&procs, &path);
        return;
    } else if ops.gui {
        let font = load_ttf_font_from_bytes(include_bytes!("../font.ttf")).unwrap();

        loop {
            let height = screen_height();
            let mut overlay = None;
            draw_pid(0, &procs, 0., height, 0, 0, &font, &mut overlay);
            if let Some(overlay) = overlay {
                draw_overlay(overlay)
            }
            next_frame().await;

            if time.elapsed().as_secs_f32() > update_rate {
                procs = update_procs(&mut sys, StatKind::Memory);
                time = Instant::now();
            }
        }
    } else if ops.tui {
        todo!()
    } else {
        println!("pid\tparent\tname\tmem self\tmem children\tmem total");
        for p in &procs {
            {
                println!(
                    "{}\t{:?}\t{}\t{}\t{}\t{}",
                    p.pid,
                    p.parent,
                    p.name,
                    p.stat,
                    p.children,
                    p.total()
                );
            }
        }
    }
}

struct ProcOverlay {
    proc: MyProcess,
    x: f32,
    y: f32,
}

fn name_with_parents(pid: usize, procs: &[MyProcess]) -> String {
    if pid == 0 {
        "root".to_string()
    } else {
        let proc = procs.iter().find(|x| x.pid == pid).unwrap();
        format!(
            "{};{}({})",
            name_with_parents(proc.parent, procs),
            proc.name,
            proc.pid
        )
    }
}

fn draw_flamegraph(procs: &[MyProcess], path: &PathBuf) {
    let mut options = inferno::flamegraph::Options::default();

    let lines = procs
        .iter()
        .map(|x| format!("{} {}", name_with_parents(x.pid, procs), x.stat))
        .collect::<Vec<String>>();
    let lines = lines.iter().map(|x| x.as_str()).collect::<Vec<&str>>();

    //let lines = ["root;bar 10", "root;bar;baz 5"];
    let flamegraph_file = File::create(path).expect("unable to create flamegraph.svg output file");

    let flamegraph_writer = BufWriter::new(flamegraph_file);
    inferno::flamegraph::from_lines(&mut options, lines, flamegraph_writer);
}

fn draw_pid(
    pid: usize,
    procs: &[MyProcess],
    starting_height: f32,
    available_height: f32,
    current_depth_total: u64,
    depth: usize,
    fonts: &Font,
    overlay: &mut Option<ProcOverlay>,
) {
    let mut total_height = available_height;
    let mut own_height = 0.;
    //draw self
    if pid > 0 {
        let t = procs.iter().find(|x| x.pid == pid).unwrap();
        let is_top = procs.iter().filter(|s| s.stat >= t.stat).count() <= 5;
        total_height = t.total() as f32 / current_depth_total as f32 * available_height;
        own_height = t.stat as f32 / current_depth_total as f32 * available_height;

        let col = if is_top {
            Color::from_rgba(255, 99, 104, 255)
        } else {
            Color::from_rgba(255, 191, 173, 255)
        };
        let width = screen_width() / 5.;
        let x = (depth - 1) as f32 * width;

        draw_process(
            x,
            width,
            starting_height,
            total_height,
            own_height,
            t,
            fonts,
            col,
        );
        //if mouse over, we show more info
        let mouse = mouse_position();
        if mouse.0 >= x
            && mouse.0 < x + width
            && mouse.1 >= starting_height
            && mouse.1 <= starting_height + total_height
        {
            *overlay = Some(ProcOverlay {
                proc: t.clone(),
                x: mouse.0,
                y: mouse.1,
            });
        }
    }

    //draw children

    let mut starting_height = starting_height;
    let mut children: Vec<&MyProcess> = procs
        .iter()
        .filter(|x| x.parent == pid)
        //.filter(|x| x.children > 10_000_000)
        .collect();
    children.sort_by(|a, b| b.total().cmp(&a.total()));
    let total = children.iter().map(|x| x.total()).sum();
    for child in children {
        draw_pid(
            child.pid,
            procs,
            starting_height,
            total_height - own_height, //dont include own
            total,
            depth + 1,
            fonts,
            overlay,
        );
        let child_height = child.total() as f32 / total as f32 * (total_height - own_height);
        starting_height += child_height;
    }
}

fn draw_overlay(overlay: ProcOverlay) {
    let ProcOverlay { proc: t, x, y } = overlay;
    draw_rectangle(x, y, 300., 100., Color::from_rgba(0, 0, 0, 100));
    draw_text(
        &t.name,
        x + 3.,
        y + 3. + 26.,
        26.,
        Color::from_rgba(255, 255, 255, 255),
    );
}

fn draw_process(
    x: f32,
    width: f32,
    current_depth_cumulative_height: f32,
    total_height: f32,
    own_height: f32,
    t: &MyProcess,
    fonts: &Font,
    col: Color,
) {
    //own + children height
    draw_rectangle(
        x,
        current_depth_cumulative_height,
        width,
        total_height,
        Color::from_rgba(50, 156, 255, 255),
    );
    //own height at bottom
    draw_rectangle(
        x,
        current_depth_cumulative_height + total_height - own_height,
        width,
        own_height,
        col,
    );
    draw_rectangle_lines(
        x,
        current_depth_cumulative_height,
        width,
        total_height,
        1.,
        Color::from_rgba(255, 255, 255, 255),
    );

    let font_height = 18.;
    let text_params = TextParams {
        font: *fonts,
        font_size: font_height as u16,
        font_scale: 1.,
        font_scale_aspect: 1.,
        rotation: 0.,
        color: Color::from_rgba(255, 255, 255, 255),
    };

    if total_height > font_height {
        draw_text_ex(
            &t.name,
            x,
            current_depth_cumulative_height + font_height,
            text_params,
        );
    }
    if total_height - own_height > font_height * 2. {
        draw_text_ex(
            &t.pid.to_string(),
            x,
            current_depth_cumulative_height + font_height * 2.,
            text_params,
        );
    }

    //text child stat
    if total_height - own_height > font_height * 3. {
        draw_text_ex(
            &t.children.formato("#,##0"),
            x,
            current_depth_cumulative_height + total_height - own_height - 5.,
            text_params,
        );
    }

    //text self
    if own_height > font_height && total_height > font_height * 2. {
        draw_text_ex(
            &t.stat.formato("#,##0"),
            x,
            current_depth_cumulative_height + total_height - 5.,
            text_params,
        );
    }
}

///we add up the value of all the children
fn update_children_usage(procs: &mut Vec<MyProcess>) {
    for i in 0..procs.len() {
        let size = value_of_children(procs[i].pid, &procs);
        procs[i].children = size;
    }
}

fn value_of_children(this_pid: usize, vec: &Vec<MyProcess>) -> u64 {
    let mut size = 0;
    for proc in vec {
        if proc.parent == this_pid && proc.pid != 0 {
            size += value_of_children(proc.pid, vec) + proc.stat;
        }
    }
    size
}

fn from_file() -> Vec<MyProcess> {
    let mut procs: Vec<MyProcess> = std::fs::read_to_string("mem.txt")
        .unwrap()
        .split('\n')
        .map(|s| {
            let mut vals = s.split('\t');

            MyProcess {
                pid: vals.nth(0).unwrap().parse().unwrap(),
                parent: vals.nth(0).unwrap().parse().unwrap(),
                name: vals.nth(0).unwrap().to_string(),
                stat: vals.nth(0).unwrap().parse().unwrap(),
                children: 0,
            }
        })
        .collect();
    update_children_usage(&mut procs);
    procs
}

enum StatKind {
    Memory,
}

fn update_procs(sys: &mut System, kind: StatKind) -> Vec<MyProcess> {
    sys.refresh_all();
    let mut procs = sys
        .processes()
        .into_iter()
        .map(|x| x.1)
        .map(|x| {
            let stat = match kind {
                StatKind::Memory => x.memory(),
            };
            MyProcess {
                pid: x.pid().into(),
                parent: x.parent().map_or(0, |f| f.into()),
                name: x.name().to_owned(),
                stat,
                children: 0,
            }
        })
        .filter(|x| x.pid != 0) //dont want root or errors
        //.filter(|x| x.pid == 9536 || x.parent == 9536 || x.parent == 0)
        .collect::<Vec<MyProcess>>();

    //if parent does not exist, we force it to be in the root
    let pids = procs.iter().map(|x| x.pid).collect::<HashSet<usize>>();
    for proc in procs.iter_mut() {
        if !pids.contains(&proc.parent) {
            proc.parent = 0;
        }
    }

    procs.sort_by(|a, b| a.stat.cmp(&b.stat));

    update_children_usage(&mut procs);

    procs
}
