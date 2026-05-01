use crate::{mynetwork::MyNetwork, myprocess::MyProcess};
use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    path::PathBuf,
    str::FromStr,
    time::Instant,
};
use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, Networks, ProcessRefreshKind, RefreshKind, System,
};

pub struct ProcManager {
    procs: Vec<MyProcess>,
    network_data: Vec<MyNetwork>,

    //sysinfo objects
    networks: Networks,
    system: System,
    last_update: Instant,
}

impl ProcManager {
    pub fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_processes(ProcessRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );

        let mut procs = update_procs(&mut system);
        //remove all disk on the first update, as they have movement
        procs.iter_mut().for_each(|p| p.disk = 0.0);
        let networks = Networks::new_with_refreshed_list();

        Self {
            procs,
            system,
            last_update: Instant::now(),
            networks,
            network_data: vec![],
        }
    }
    pub fn update(&mut self) {
        self.procs = update_procs(&mut self.system);
        //calc writes per second
        self.procs.iter_mut().for_each(|p| {
            p.disk /= Instant::now()
                .saturating_duration_since(self.last_update)
                .as_secs_f64()
        });
        self.update_network_data();
        self.last_update = Instant::now();
    }
    pub fn procs(&self) -> &Vec<MyProcess> {
        &self.procs
    }

    pub fn get_networks(&self) -> Vec<MyNetwork> {
        self.network_data.clone()
    }

    pub fn get_totals(&self) -> Totals {
        //there is a difference between the sum of the procs resident memory and total memory as per sysinfo.
        //we use sum of proc resident memory to be consistent with proc display
        let memory_procs = self.procs.iter().map(|x| x.memory).sum();
        let memory_used = self.system.used_memory();
        //sometimes on macos cpu is nan
        let cpus: Vec<f32> = self
            .system
            .cpus()
            .iter()
            .map(|cpu| cpu.cpu_usage())
            .collect();
        let cpu_count = cpus.len();
        let cpu_total: f32 = cpus.iter().sum::<f32>().zero_if_nan();

        let cpu_avg = cpu_total / cpu_count as f32;

        Totals {
            memory_procs,
            memory_used,
            cpu_avg,
            cpu_count,
            cpus,
            uptime: System::uptime(),
            memory_total: self.system.total_memory(),
        }
    }

    fn update_network_data(&mut self) {
        self.networks.refresh(true);
        let elapsed = Instant::now()
            .saturating_duration_since(self.last_update)
            .as_secs_f64() as u64;
        for n in self.networks.iter().filter(|n| n.0 != "lo") {
            let received_per_sec = n.1.received() / 1.max(elapsed);
            let sent_per_sec = n.1.transmitted() / 1.max(elapsed);

            if let Some(net) = self.network_data.iter_mut().find(|a| &a.name == n.0) {
                net.received += n.1.received();
                net.sent += n.1.transmitted();
                net.received_per_sec = received_per_sec;
                net.sent_per_sec = sent_per_sec;
            } else {
                self.network_data.push(MyNetwork {
                    name: n.0.to_string(),
                    received: n.1.received(),
                    sent: n.1.transmitted(),
                    received_per_sec: received_per_sec,
                    sent_per_sec: sent_per_sec,
                });
            }
        }
    }
}

trait NoNan {
    fn zero_if_nan(self) -> Self;
}
impl NoNan for f32 {
    fn zero_if_nan(self) -> Self {
        if self.is_nan() { 0. } else { self }
    }
}

pub struct Totals {
    pub memory_procs: u64,
    pub memory_used: u64,
    pub memory_total: u64,
    pub cpu_avg: f32,
    pub cpu_count: usize,
    pub uptime: u64,
    pub cpus: Vec<f32>,
}

///we add up the value of all the children
fn update_children_usage(procs: &mut Vec<MyProcess>) {
    // pid -> index
    let index_map: HashMap<usize, usize> =
        HashMap::from_iter(procs.iter().enumerate().map(|(i, p)| (p.pid, i)));

    // process deepest first so children accumulate before parents
    let mut indices: Vec<usize> = (0..procs.len()).collect();
    indices.sort_by_key(|&a| Reverse(procs[a].depth));

    for &i in &indices {
        let parent = procs[i].parent;
        if parent == 0 {
            continue;
        }
        //add itself plus its own children to the parent
        if let Some(&parent_idx) = index_map.get(&parent) {
            procs[parent_idx].children_memory += procs[i].memory + procs[i].children_memory;
        }
    }
}

fn update_procs(sys: &mut System) -> Vec<MyProcess> {
    sys.refresh_all();
    let mut procs = sys
        .processes()
        .iter()
        .map(|x| x.1)
        .filter(|x| x.thread_kind() != Some(sysinfo::ThreadKind::Userland))
        .map(|x| {
            let long_cmd: Vec<String> = x
                .cmd()
                .iter()
                .map(|os| os.clone().into_string().unwrap_or_default())
                .collect();
            let cmd = match x.exe() {
                Some(s) if s.as_os_str().is_empty() => x.name().to_string_lossy().to_string(),
                Some(s) => s.to_string_lossy().to_string(),
                None => x.name().to_string_lossy().to_string(),
            };
            let full_cmd = if long_cmd.is_empty() {
                cmd.clone()
            } else {
                let long_cmd_path = PathBuf::from_str(&long_cmd[0]).unwrap_or_default();
                let long_cmd_path = long_cmd_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let cmd_path = PathBuf::from_str(&cmd).unwrap_or_default();
                let cmd_path = cmd_path.file_name().unwrap_or_default().to_string_lossy();

                if long_cmd_path == cmd_path {
                    format!("{cmd_path} {}", long_cmd[1..].join(" "))
                } else {
                    format!("{cmd_path}|{}", long_cmd.join(" "))
                }
            };
            // let full_cmd = format!("{cmd} | {}", long_cmd.join(" "));

            MyProcess {
                pid: x.pid().into(),
                parent: x.parent().map_or(0, |f| f.into()),
                name: x.name().to_string_lossy().to_string(),
                command: long_cmd.join(" "),
                command_display: full_cmd,
                memory: x.memory(),
                cpu: x.cpu_usage(),
                children_memory: 0,
                run_time: x.run_time(),
                depth: 0,
                disk: (x.disk_usage().read_bytes + x.disk_usage().written_bytes) as f64,
            }
        })
        .filter(|x| x.pid != 0) //dont want root or errors
        .collect::<Vec<MyProcess>>();

    //break cycle loops parent 1 -> child 2 -> parent 1
    //not sure why this happens, possibly reuse of pid's
    //if there is a loop, we make the lowest pid's parent, equal to 0
    //using lowest pid is not entirely correct, but we must break cycle somewhere
    //TODO: find actual parent which no longer exists
    let mut change = HashSet::new();
    for proc in procs.iter() {
        let parents = parents_limited(proc.pid, &procs, HashSet::new());
        //if parents contains this pid, this pid is looped, so we make it 0 if it is the lowest
        if parents.contains(&proc.pid) && parents.iter().min().unwrap() == &proc.pid {
            change.insert(proc.pid);
        }
    }
    for proc in procs.iter_mut() {
        if change.contains(&proc.pid) {
            proc.parent = 0;
        }
    }

    add_depths(&mut procs);

    //if parent does not exist, we force it to be in the root
    let pids = procs.iter().map(|x| x.pid).collect::<HashSet<usize>>();
    for proc in procs.iter_mut() {
        if !pids.contains(&proc.parent) {
            proc.parent = 0;
        }
    }

    procs.sort_by_key(|a| Reverse(a.memory));

    update_children_usage(&mut procs);

    procs
}

///add depths to processes
//we process each item, and walk up to parent to count the steps to root, this is out depth
//to speed up, we cache the depth of each item's parent when we visit them the first time, so we dont need to walk them again
fn add_depths(procs: &mut Vec<MyProcess>) {
    let index_map: HashMap<usize, usize> =
        HashMap::from_iter(procs.iter().enumerate().map(|(i, p)| (p.pid, i)));
    //calc depth
    for pi in 0..procs.len() {
        let mut current = &procs[pi];
        let mut depths = vec![current.pid]; //depths of items we need to save
        let mut depth = 1; //actual depth
        while let Some(p) = index_map.get(&current.parent) {
            let parent_depth = procs[*p].depth;
            if parent_depth != 0 {
                //we have already the depth of the parent, use itls
                depth += parent_depth;
                break;
            } else {
                depth += 1;
                depths.push(current.parent);
                current = &procs[*p];
            }
        }
        for i in 0..depths.len() {
            let index = index_map.get(&depths[i]).unwrap();
            procs[*index].depth = depth - i;
        }
    }
}

///This finds all parents
/// if there is a loop, it returns
fn parents_limited(pid: usize, procs: &[MyProcess], mut parents: HashSet<usize>) -> HashSet<usize> {
    let proc = procs.iter().find(|x| x.pid == pid);
    let parent = if let Some(proc) = proc {
        proc.parent
    } else {
        0
    };

    if parents.contains(&parent) {
        parents
    } else {
        parents.insert(parent);
        parents_limited(parent, procs, parents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: usize, parent: usize, memory: u64, depth: usize) -> MyProcess {
        MyProcess {
            pid,
            parent,
            memory,
            depth,
            ..Default::default()
        }
    }

    #[test]
    fn test_child_mem_and_depths() {
        // flat: 1(100) -> 2(50), 3(30), 4(20)
        let mut procs = vec![
            proc(1, 0, 100, 0),
            proc(2, 1, 50, 0),
            proc(3, 1, 30, 0),
            proc(4, 1, 20, 0),
        ];
        add_depths(&mut procs);
        assert_eq!(
            procs.iter().map(|p| p.depth).collect::<Vec<_>>(),
            vec![1, 2, 2, 2]
        );
        update_children_usage(&mut procs);
        assert_eq!(procs[0].children_memory, 100); // 50+30+20

        // deep chain: 1(10) -> 2(20) -> 3(30) -> 4(40)
        let mut procs = vec![
            proc(1, 0, 10, 0),
            proc(2, 1, 20, 0),
            proc(3, 2, 30, 0),
            proc(4, 3, 40, 0),
        ];
        add_depths(&mut procs);
        assert_eq!(
            procs.iter().map(|p| p.depth).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        update_children_usage(&mut procs);
        assert_eq!(procs[2].children_memory, 40); // 3 -> 4
        assert_eq!(procs[1].children_memory, 70); // 2 -> 3 -> 4
        assert_eq!(procs[0].children_memory, 90); // 1 -> 2 -> 3 -> 4

        // mixed: 1(10) -> 2(20) -> 4(40), 2(20) -> 5(50), 1(10) -> 3(30)
        let mut procs = vec![
            proc(1, 0, 10, 0),
            proc(2, 1, 20, 0),
            proc(3, 1, 30, 0),
            proc(4, 2, 40, 0),
            proc(5, 2, 50, 0),
        ];
        add_depths(&mut procs);
        assert_eq!(
            procs.iter().map(|p| p.depth).collect::<Vec<_>>(),
            vec![1, 2, 2, 3, 3]
        );
        update_children_usage(&mut procs);
        assert_eq!(procs[1].children_memory, 90); // 2 -> 4+5
        assert_eq!(procs[0].children_memory, 140); // 1 -> 2+3 (+ 4+5)
    }
}
