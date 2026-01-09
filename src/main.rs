mod config;
#[cfg(feature = "gui")]
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

use crate::config::{Config, Mode};

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
    let config = Config::load();
    let default_mode = config.mode;

    if let Some(path) = ops.svg {
        draw_flamegraph(path);
    } else if ops.gui {
        run_gui();
    } else if ops.tui {
        run_tui(config);
    } else if ops.out {
        run_output();
    } else {
        //no arguments so we try load config or default
        match default_mode {
            Mode::Gui => run_gui(),
            Mode::Tui => run_tui(config),
        }
    }
}

fn run_gui() {
    if cfg!(feature = "gui") {
        #[cfg(feature = "gui")]
        gui::run();
    } else {
        println!("gui feature not enabled");
    }
}

///run tui, might also request to go to gui within tui
fn run_tui(config: Config) {
    match tui::run(config) {
        Ok(true) => gui::run(),
        Ok(false) => {}
        Err(err) => {
            println!("error: {}", err);
        }
    }
}

fn run_output() {
    let man = manager::ProcManager::new();
    let procs = man.procs();
    println!("pid\tparent\tname\tmemself\tmemchildren\tmemtotal");
    for p in procs {
        {
            println!(
                "{}\t{:?}\t{}\t{}\t{}\t{}\t{}",
                p.pid,
                p.parent,
                p.name,
                p.memory,
                p.children_memory,
                p.total(),
                p.cpu
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
