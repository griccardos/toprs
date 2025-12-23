use std::time::Duration;

use dioxus::{document::eval, prelude::*};
use dioxus_desktop::{Config, WindowBuilder, tao::window::Icon, wry::dpi::PhysicalSize};

use crate::{
    helpers::{nice_size_g_thousands, nice_size_thousands, nice_time},
    manager::ProcManager,
    myprocess::MyProcess,
    sorted::{SortType, SortedProcesses},
    svgmaker,
};

///Hide console window because we are running gui
#[cfg(target_os = "windows")]
fn hide_console_window() {
    use winapi::um::wincon::GetConsoleWindow;
    use winapi::um::winuser::{SW_HIDE, ShowWindow};
    let window = unsafe { GetConsoleWindow() };
    if !window.is_null() {
        unsafe {
            ShowWindow(window, SW_HIDE);
        }
    }
}

pub fn run() {
    #[cfg(target_os = "windows")]
    hide_console_window();

    let index = include_str!("index.html").to_string();
    let index = index.replace("PLOTLYSCRIPT", include_str!("plotly-2.18.0.min.js"));

    let config = Config::default().with_custom_index(index).with_window(
        WindowBuilder::new()
            .with_title("toprs")
            .with_inner_size(PhysicalSize::new(1500, 1000))
            .with_window_icon(Some(load_icon())),
    );
    dioxus_desktop::launch::launch(app, vec![], vec![Box::new(config)]);
}

fn app() -> Element {
    let mut man = use_signal(ProcManager::new);
    let mut my_svg = use_signal(|| "".to_string());
    let top5 = get_top5(man.read().procs());
    let mut max_depth = use_signal(|| top5.iter().map(|x| x.depth).max().unwrap_or(5));
    let totals = man.read().get_totals();
    let mem = nice_size_g_thousands(totals.memory);
    let totmem = nice_size_g_thousands(totals.memory_total);
    let cpu = format!("{:5>.1}% x{}", totals.cpu_avg, totals.cpu_count);
    let cpupercent = format!("{:.1}%", totals.cpu_avg);
    let mempercent = format!(
        "{:.1}%",
        totals.memory as f64 / totals.memory_total as f64 * 100.
    );
    let uptime = nice_time(totals.uptime);
    let mut live = use_signal(|| true);
    let mut visible = use_signal(SortedProcesses::new);
    let processes = use_signal(|| man.read().procs().len());

    update_sunburst(man, max_depth);

    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        loop {
            if *live.read() {
                man.with_mut(|s| s.update());
                my_svg.set(svgmaker::generate_svg(man.read().procs()));
                visible.write().update(man.read().procs());
            }
            tokio::time::sleep(Duration::from_millis(2000)).await;
        }
    });

    rsx! {
        table {
            tr {
                td { width: "100px", "Memory" }
                td { class: "tot", "{mem}/{totmem}" }
                td { class: "col3",
                    div { class: "loading-bar", div { class: "progress", width: "{mempercent}" } }
                }
            }
            tr {
                td { "Cpu" }
                td { class: "tot", "{cpu}" }
                td { class: "col3",
                    div { class: "loading-bar", div { class: "progress", width: "{cpupercent}" } }
                }
            }
            tr {
                td { "Uptime" }
                td { class: "tot", "{uptime}" }
            }
            tr {
                td { "Processes" }
                td { class: "tot", "{processes}" }
            }
        }
        div {
            "Filter"
            input {
                style: "margin-left:20px",
                oninput: move |a| {
                    visible.write().set_filter(a.value().clone());
                }
            }
        }
        div { style: "height:300px;overflow:auto",
            table { class: "tproc",
                thead {
                    tr { class: "thead",
                        for (i , p) in ["Command", "Name", "PID", "Self", "Children", "Total", "CPU"].iter().enumerate() {
                            td {
                            onclick: move |_| {
                                    if visible.read().sort_col == i {
                                        visible.write().sort_cycle();
                                    } else {
                                        visible.write().sort_col = i;
                                    }
                                    if visible.read().sort_type == SortType::None && i > 0 {
                                        visible.write().sort_cycle();
                                    }
                                    visible.write().update(man.read().procs());
                                },
                                style: if i == 0 { "width:700px" } else{"width:90px" },
                                class: if i < 2 { "" } else { "tright" },
                                "{sort_name(p,i,visible)}"
                            }
                        }
                    }
                }
                tbody {
                    for pr in visible.read().procs().iter() {
                        tr {
                            td{title:"{pr[0]}",class:"tcell ","{pr[0]}"}
                            td{title:"{pr[1]}",class:"tcell ","{pr[1]}"}
                            td{class:"tcell tright","{pr[2]}"}
                            td{class:"tcell tright","{pr[3]}"}
                            td{class:"tcell tright","{pr[4]}"}
                            td{class:"tcell tright","{pr[5]}"}
                            td{class:"tcell tright","{pr[6]}"}
                        }
                    }
                }
            }
        }
        h2 { "Memory analysis" }
        div {
            "Live update"
            input {
                style: "margin-left:20px",
                r#type: "checkbox",
                checked: "{live}",
                oninput: move |_| {
                    let old = *live.read();
                    live.set(!old);
                }
            }
        }

        div {
            "Max depth:"
            input {
                style: "margin-left:20px",
                r#type: "number",
                value: "{max_depth}",
                oninput: move |a| {
                    let val = a.value().parse::<usize>().unwrap_or(100);
                    max_depth.set(val)
                }
            }
        }
        div { id: "myDiv" }
        div { dangerous_inner_html: "{my_svg}" }
    }
}

fn update_sunburst(
    man: Signal<ProcManager>,
    //eval: &std::rc::Rc<dyn Fn(&str) -> Result<UseEval, EvalError>>,
    max: Signal<usize>,
) {
    let (l, p, v, t, m, c) = get_labels_parents_values(man.read().procs());
    let js = r##"

     var data = [{
         type: "sunburst",
         maxdepth: MAXDEPTHVALUE,
         labels: [LABELS],
         parents: [PARENTS],
         values:  [VALUES],
         meta: [META],
         text:[TEXT],
         //hoverinfo: "label+value+percent root",
         hovertemplate:"%{text}  %{percentRoot:.1%} with children %{meta} of total <extra></extra>",
         outsidetextfont: {size: 20, color: "#377eb8"},
         leaf: {opacity: 1.0},
         marker: {line: {width: 2},colors:[COLORS]},
         branchvalues:"total",
         
         }];
         
         
         var layout = {
         margin: {l: 0, r: 0, b: 0, t: 0},
         plot_bgcolor:"black",
         paper_bgcolor:"rgba(0,0,0,0.3)",
         width: 1400,
         height: 1000
         };
         Plotly.newPlot('myDiv',data,layout,{displaylogo: false});
     "##;
    let js = js
        .replace("LABELS", &l)
        .replace("PARENTS", &p)
        .replace("VALUES", &v)
        .replace("TEXT", &t)
        .replace("META", &m)
        .replace("COLORS", &c)
        .replace("MAXDEPTHVALUE", &max.read().to_string());
    let _ = eval(&js);
}

fn proc_label(proc: &MyProcess) -> String {
    format!("{} {} ", proc.name, proc.pid,)
}

fn get_labels_parents_values(
    procs: &[MyProcess],
) -> (String, String, String, String, String, String) {
    let labels = procs
        .iter()
        .map(|f| format!(r##""{}""##, proc_label(f)))
        .collect::<Vec<String>>()
        .join(",");
    let parents = procs
        .iter()
        .map(|f| {
            let n = if f.parent == 0 {
                "".to_string()
            } else {
                let proc = procs.iter().find(|k| k.pid == f.parent).unwrap();
                proc_label(proc)
            };
            format!(r##""{n}""##)
        })
        .collect::<Vec<String>>()
        .join(",");
    let values = procs
        .iter()
        .map(|f| f.total().to_string())
        .collect::<Vec<String>>()
        .join(",");

    let selfs = procs
        .iter()
        .map(|f| {
            format!(
                r#""{} own:{} total:{}""#,
                f.name,
                nice_size_thousands(f.memory),
                nice_size_thousands(f.total()),
            )
        })
        .collect::<Vec<String>>()
        .join(",");

    //let fifth_largest: u64 = get_top5(procs).last().unwrap().memory;
    let top = get_top_memory(procs).memory;

    let colors = procs
        .iter()
        .map(|t| {
            //color from red to light yellow based on memory usage compared to top
            let ratio = (t.memory as f32 / top as f32).powf(0.3);
            let g = ((1.0 - ratio) * 255.0) as u32;
            let b = ((1.0 - ratio) * 150.0) as u32;
            let col = format!("rgb(255, {g}, {b})");

            format!(r#""{}""#, col)
        })
        .collect::<Vec<String>>()
        .join(",");

    let total = procs.iter().map(|x| x.memory).sum::<u64>();
    let meta = procs
        .iter()
        .map(|f| format!(r#""{:.1}%""#, (100. * f.memory as f64 / total as f64)))
        .collect::<Vec<String>>()
        .join(",");

    (labels, parents, values, selfs, meta, colors)
}

fn get_top5(procs: &[MyProcess]) -> Vec<MyProcess> {
    let mut temp = procs.to_vec();
    temp.sort_by(|a, b| b.memory.cmp(&a.memory));
    temp.iter().take(5).cloned().collect()
}
fn get_top_memory(procs: &[MyProcess]) -> &MyProcess {
    procs.iter().max_by_key(|a| a.memory).unwrap()
}

fn sort_name(name: &str, col: usize, sorted: Signal<SortedProcesses>) -> String {
    let sym = if col == sorted.read().sort_col {
        match sorted.read().sort_type {
            SortType::Ascending => "↑",
            SortType::Descending => "↓",
            _ => "",
        }
    } else {
        ""
    };

    format!("{name}{sym}")
}

fn load_icon() -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(include_bytes!("../icon.png"))
            .unwrap()
            .resize(32, 32, image::imageops::FilterType::Gaussian)
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };

    Icon::from_rgba(icon_rgba, icon_width, icon_height).unwrap()
}
