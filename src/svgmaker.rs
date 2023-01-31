use std::cmp::Reverse;

use crate::{helpers::nice_size, myprocess::MyProcess};

static ROOT: &str = r##"<?xml version="1.0" standalone="no"?><!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg version="1.1" width="99%" height="100%" viewBox="0 0 1000 300" xmlns="http://www.w3.org/2000/svg"  >
<style type="text/css">
text { font-family:monospace; font-size:14px }
g.hoverable:hover { stroke:black; stroke-width:1; cursor:pointer; }
</style>

  [SVG]
</svg>
"##;

pub fn generate_svg(procs: &[MyProcess]) -> String {
    let mut svg = r##"<svg id="data" x="10" y="10" width="1000">"##.to_string();

    let height = 20.;
    let width = 1000.; //width as f32;

    let mut layers = vec![vec![]];
    draw_pid(0, procs, 0., width, height, 0, 0, &mut layers);
    let total_stat: u64 = procs.iter().map(|s| s.memory).sum();
    for layer in layers {
        for item in layer {
            if item.total_width <= 0. {
                continue;
            }
            let percent_total =
                format!("{:.2}%", item.proc.memory as f32 / total_stat as f32 * 100.);
            let text = if item.total_width > 10. {
                format!(
                    r#"<text x="{}" y="{}">{} {}</text>"#, //text
                    item.x,
                    item.y + 15.,
                    item.proc.name,
                    item.proc.pid,
                )
            } else {
                "".to_string()
            };

            svg.push_str(&format!(
                r#"<g class="hoverable">
                <title>{} {} {} (own: {} = {}; including childen: {})</title>
                <rect  x="{}" y="{}" width="{}" height="{}" fill="{}"   />
                <rect  x="{}" y="{}" width="{}" height="{}" fill="{}" style="stroke-width:0" />
                {text}
                </g> 
                <rect x="{}" y="{}" width="{}" height="{}" fill="{}"  />
                "#,
                //title for hover
                item.proc.name,
                item.proc.pid,
                item.proc.command,
                nice_size(item.proc.memory),
                percent_total,
                nice_size(item.proc.total()),
                //child rect
                item.x,
                item.y,
                item.total_width,
                height,
                "rgb(190, 196, 255)",
                //own rect
                item.x + item.children_width,
                item.y + 0.25,
                item.own_width - 0.25,
                height - 0.5,
                item.col,
                //whiteout rest
                item.x + item.total_width,
                item.y,
                width - item.x - item.total_width,
                height,
                "black"
            ));
        }
    }

    svg.push_str("</svg>");

    ROOT.replace("1000", &width.to_string())
        .replace("[SVG]", &svg)
}

struct LayerProc {
    x: f32,
    total_width: f32,
    children_width: f32,
    own_width: f32,
    y: f32,
    col: String,
    proc: MyProcess,
}

fn draw_pid(
    pid: usize,
    procs: &[MyProcess],
    starting_width: f32,
    available_width: f32,
    height: f32,
    mut current_depth_total: u64,
    depth: usize,
    layers: &mut Vec<Vec<LayerProc>>,
) {
    let mut vals: Vec<u64> = procs.iter().map(|s| s.memory).collect();
    vals.sort_by(|a, b| b.cmp(a));
    let fifth_largest: u64 = vals.iter().skip(4).copied().next().unwrap_or_default();

    let to;
    let t;
    if pid == 0 {
        let tot: u64 = procs
            .iter()
            .filter(|s| s.parent == 0)
            .map(|s| s.total())
            .sum();
        to = MyProcess {
            pid: 0,
            parent: 0,
            name: "all".to_string(),
            memory: 0,
            cpu: 0.,
            children_memory: tot,
            command: "".to_string(),
            depth: 0,
        };
        t = &to;

        current_depth_total = tot;
    } else {
        t = procs.iter().find(|x| x.pid == pid).unwrap();
    }

    //add self

    let is_top = t.memory >= fifth_largest;
    let total_width = t.total() as f32 / current_depth_total as f32 * available_width;
    let own_width = t.memory as f32 / current_depth_total as f32 * available_width;

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
    let y = depth as f32 * (height + 1.);

    if layers.len() <= depth {
        layers.push(vec![]);
    }

    layers[depth].push(LayerProc {
        y,
        col,
        x: starting_width,
        children_width: total_width - own_width,
        own_width,
        total_width,
        proc: t.clone(),
    });

    //draw children

    let mut starting_width = starting_width;
    let mut children: Vec<&MyProcess> = procs.iter().filter(|x| x.parent == pid).collect();
    children.sort_by_key(|a| Reverse(a.total()));
    let total = children.iter().map(|x| x.total()).sum();
    for child in children {
        draw_pid(
            child.pid,
            procs,
            starting_width,
            total_width - own_width, //dont include own
            height,
            total,
            depth + 1,
            layers,
        );
        let child_width = child.total() as f32 / total as f32 * (total_width - own_width);
        starting_width += child_width;
    }
}
