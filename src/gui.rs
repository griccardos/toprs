use std::time::Duration;

use dioxus::prelude::*;
use dioxus_desktop::{tao::window::Icon, use_eval, Config, PhysicalSize, WindowBuilder};

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
    use winapi::um::winuser::{ShowWindow, SW_HIDE};
    let window = unsafe { GetConsoleWindow() };
    if !window.is_null()  {
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
    dioxus_desktop::launch_cfg(app, config);
}

fn app(cx: Scope) -> Element {
    let man = use_ref(cx, ProcManager::new);
    let my_svg = use_state(cx, || "".to_string());
    let top5 = get_top5(man.read().procs());
    let max_depth = use_state(cx, || top5.iter().map(|x| x.depth).max().unwrap_or(5));
    let totals = man.read().get_totals();
    let mem = nice_size_g_thousands(totals.memory);
    let totmem = nice_size_g_thousands(totals.memory_total);
    let cpu = format!("{:.1}%", totals.cpu);
    let uptime = nice_time(totals.uptime);
    let eval = use_eval(cx);
    let live = use_state(cx, || true);
    let visible = use_ref(cx, SortedProcesses::new);

    update_sunburst(man, eval, max_depth);

    use_coroutine(cx, |_: UnboundedReceiver<()>| {
        let my_svg = my_svg.clone();
        let man = man.clone();
        let live = live.clone();
        let visible = visible.clone();

        async move {
            loop {
                if *live.current() {
                    man.with_mut(|s| s.update());
                    my_svg.set(svgmaker::generate_svg(man.read().procs()));
                    visible.write().update(man.read().procs());
                }
                tokio::time::sleep(Duration::from_millis(5000)).await;
            }
        }
    });

    cx.render(rsx!(
        table{
            tr{td{width:"100px", "Memory"},td{class:"tot","{mem}/{totmem}"}}
            tr{td{ "Cpu"},td{class:"tot","{cpu}"}}
            tr{td{"Uptime"},td{class:"tot","{uptime}"}}
        }

        div{
            style:"height:300px;overflow:auto",
        table{
            class:"tproc",
            thead{
                tr{
                    class:"thead",
                    for (i,p) in ["Command","Name","PID","Memory","Children","Total","CPU"].iter().enumerate(){
                        td{onclick:move |_|{
                            if visible.read().sort_col==i{
                            visible.write().sort_cycle();
                            }else{
                            visible.write().sort_col=i;
                            }
                            if visible.read().sort_type==SortType::None && i>0{
                                visible.write().sort_cycle();
                            }
                            visible.write().update(man.read().procs());
                        },
                        style: if i==0{"width:700px"}else if i==1{"width:266px"}else{"width:90px"},
                        class: if i<2{""}else{"tright"}, 
                        sort_name(p,i,visible)
                    }
                    }
            }
            }
            tbody{
                    for pr in  visible.read().procs(){
                tr{
                        rsx!(
                            td{title:"{pr[0]}",class:"tcell ","{pr[0]}"}
                            td{title:"{pr[1]}",class:"tcell ","{pr[1]}"}
                            td{class:"tcell tright","{pr[2]}"}
                            td{class:"tcell tright","{pr[3]}"}
                            td{class:"tcell tright","{pr[4]}"}
                            td{class:"tcell tright","{pr[5]}"}
                            td{class:"tcell tright","{pr[6]}"}
                        )
                    }
                }

            }

        }
        }

        h2 {"Memory analysis"}
        div{
            "Live update"
        input{ 
            style:"margin-left:20px",
            r#type: "checkbox",
            checked:"{live}",
            oninput:move|_|{
                let old=*live.current();
                live.set(!old);
            }

        }
        }

        div{"Max depth:",
            input{
                style:"margin-left:20px",
                r#type:"number",
                value:"{max_depth}",
                oninput:move |a|{
                    let val = a.value.parse::<usize>().unwrap_or(100);
                    max_depth.set(val)
                }
            }
        }
           div{
               id:"myDiv",
        },
        div{
            dangerous_inner_html:"{my_svg}"
        }


    ))
}

fn update_sunburst(
    man: &UseRef<ProcManager>,
    eval: &std::rc::Rc<dyn Fn(String) -> dioxus_desktop::EvalResult>,
    max: &UseState<usize>,
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
        .replace("MAXDEPTHVALUE", &max.get().to_string());
    eval(js);
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
                r#""own:{} total:{}""#,
                nice_size_thousands(f.memory),
                nice_size_thousands(f.total())
            )
        })
        .collect::<Vec<String>>()
        .join(",");

    let fifth_largest: u64 = get_top5(procs).last().unwrap().memory;

    let colors = procs
        .iter()
        .map(|t| {
            let is_top = t.memory >= fifth_largest;

            let col = if is_top {
                "rgb(255, 99, 104)".to_string()
            } else {
                //yellow if low g = 255, b = 200
                //orange if heigh g= 190 b = 0
                let ratio = t.memory as f32 / fifth_largest as f32;
                let g = 255 - ((255 - 200) as f32 * ratio) as u32;
                let b = 190 - ((190) as f32 * ratio) as u32;

                format!("rgb(255, {g}, {b})")
            };

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

fn sort_name(name: &str, col: usize, sorted: &UseRef<SortedProcesses>) -> String {
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
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };

    Icon::from_rgba(icon_rgba, icon_width, icon_height).unwrap()
}
