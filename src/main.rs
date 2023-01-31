mod gui;
mod helpers;
mod manager;
mod myprocess;
mod sorted;
mod svgmaker;
mod tui;

use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

use gumdrop::Options;

#[derive(Options)]
struct Args {
    #[options(help = "Output to svg", meta = "<FILE>")]
    svg: Option<PathBuf>,

    #[options(help = "Show gui")]
    gui: bool,

    #[options(help = "Show tui (default)")]
    tui: bool,

    #[options(help = "Print to stdout")]
    out: bool,

    #[options(help = "Print help message")]
    help: bool,
}

fn main() {
    let ops = Args::parse_args_default_or_exit();

    if let Some(path) = ops.svg {
        draw_flamegraph(path);
    } else if ops.gui {
        gui::run();
    } else if ops.tui {
        if let Ok(run_gui) = tui::run() {
            if run_gui {
                gui::run();
            }
        }
    } else if ops.out {
        run_output();
    } else {
        tui::run().unwrap_or_default();
    }
}

fn run_output() {
    let man = manager::ProcManager::new();
    let procs = man.procs();
    println!("pid\tparent\tname\tmemself\tmemchildren\tmemtotal");
    for p in procs {
        {
            println!(
                "{}\t{:?}\t{}\t{}\t{}\t{}",
                p.pid,
                p.parent,
                p.name,
                p.memory,
                p.children_memory,
                p.total()
            );
        }
    }
}

fn draw_flamegraph(path: PathBuf) {
    let man = manager::ProcManager::new();
    let procs = man.procs();
    let file = File::create(path).expect("unable to create svg output file");
    let mut writer = BufWriter::new(file);
    let svg = svgmaker::generate_svg(procs);
    let _ = writer.write(svg.as_bytes());
}
