use std::collections::HashSet;

use sysinfo::{CpuExt, CpuRefreshKind, ProcessExt, RefreshKind, System, SystemExt};

use crate::myprocess::MyProcess;

pub struct ProcManager {
    procs: Vec<MyProcess>,
    system: System,
}

impl ProcManager {
    pub fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(),
        );

        let procs = update_procs(&mut system);

        Self { procs, system }
    }
    pub fn update(&mut self) {
        self.procs = update_procs(&mut self.system);
    }
    pub fn procs(&self) -> &Vec<MyProcess> {
        &self.procs
    }

    pub fn get_totals(&self) -> Totals {
        //there is a difference between the sum of the procs resident memory and total memory as per sysinfo.
        //we use sum of proc resident memory to be consistent with proc display
        let memory = self.procs.iter().map(|x| x.memory).sum();
        //sometimes on macos cpu is nan
        let mut cpu = self.system.global_cpu_info().cpu_usage();
        if cpu.is_nan() {
            cpu = 0.;
        }
        Totals {
            memory,
            cpu,
            uptime: self.system.uptime(),
            memory_total: self.system.total_memory(),
        }
    }
}

pub struct Totals {
    pub memory: u64,
    pub memory_total: u64,
    pub cpu: f32,
    pub uptime: u64,
}

///we add up the value of all the children
fn update_children_usage(procs: &mut Vec<MyProcess>) {
    for i in 0..procs.len() {
        let size = sum_of_children(procs[i].pid, procs);
        procs[i].children_memory = size;
    }
}

fn sum_of_children(this_pid: usize, vec: &Vec<MyProcess>) -> u64 {
    let mut size = 0;
    for proc in vec {
        if proc.parent == this_pid && proc.pid != 0 {
            size += sum_of_children(proc.pid, vec) + proc.memory;
        }
    }
    size
}

fn update_procs(sys: &mut System) -> Vec<MyProcess> {
    sys.refresh_all();
    let mut procs = sys
        .processes()
        .iter()
        .map(|x| x.1)
        .map(|x| {
            let cmd = if x.exe().as_os_str().is_empty() {
                x.name().to_owned()
            } else {
                x.exe().to_string_lossy().to_string()
            };
            MyProcess {
                pid: x.pid().into(),
                parent: x.parent().map_or(0, |f| f.into()),
                name: x.name().to_owned(),
                command: cmd,
                memory: x.memory(),
                cpu: x.cpu_usage(),
                children_memory: 0,
                depth: 0,
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

    //calc depth
    for pi in 0..procs.len() {
        let pid = procs[pi].pid;
        procs[pi].depth = depth(pid, &procs);
    }

    //if parent does not exist, we force it to be in the root
    let pids = procs.iter().map(|x| x.pid).collect::<HashSet<usize>>();
    for proc in procs.iter_mut() {
        if !pids.contains(&proc.parent) {
            proc.parent = 0;
        }
    }

    procs.sort_by(|a, b| b.memory.cmp(&a.memory));

    update_children_usage(&mut procs);

    procs
}
fn depth(pid: usize, procs: &[MyProcess]) -> usize {
    let mut depth = 0usize;
    let mut pid = pid;
    loop {
        let proc = procs.iter().find(|p| p.pid == pid);

        if let Some(proc) = proc {
            depth += 1;
            pid = proc.parent;
        } else {
            return depth;
        }
        if depth > 1000 {
            return 0;
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
