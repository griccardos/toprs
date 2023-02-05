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
    str::FromStr,
};

use gumdrop::Options;
use serde::Deserialize;

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

#[derive(Deserialize)]
enum Mode {
    Gui,
    Tui,
}

#[derive(Deserialize)]
struct Config {
    mode: Mode,
}

fn main() {
    let ops = Args::parse_args_default_or_exit();
    let config = get_config();

    if let Some(path) = ops.svg {
        draw_flamegraph(path);
    } else if ops.gui {
        gui::run();
    } else if ops.tui {
        run_tui();
    } else if ops.out {
        run_output();
    } else {
        //no arguments so we try load config or default
        match config.mode {
            Mode::Gui => gui::run(),
            Mode::Tui => run_tui(),
        }
    }
}

///run tui, might also request to go to gui within tui
fn run_tui() {
    if let Ok(run_gui) = tui::run() {
        if run_gui {
            gui::run();
        }
    }
}

fn get_config() -> Config {
    //load paths: first home, else /etc
    let mut paths = vec![];
    //home directory
    if let Some(mut dir) = dirs::home_dir() {
        dir.push(".config");
        dir.push("toprs");
        dir.push("config.toml");
        paths.push(dir);
    }
    //fallback
    paths.push(PathBuf::from_str("/etc/toprs/config.toml").unwrap());

    //if find file, use it
    for path in paths {
        if path.exists() {
            if let Ok(str) = std::fs::read_to_string(path) {
                if let Ok(config) = toml::from_str(&str) {
                    return config;
                }
            }
        }
    }

    //default
    Config { mode: Mode::Tui }
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
