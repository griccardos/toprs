use std::collections::HashSet;

use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

use crate::myprocess::MyProcess;

pub struct ProcManager {
    procs: Vec<MyProcess>,
    system: System,
}

impl ProcManager {
    pub fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
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
            memory,
            cpu_avg,
            cpu_count,
            cpus,
            uptime: System::uptime(),
            memory_total: self.system.total_memory(),
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
    pub memory: u64,
    pub memory_total: u64,
    pub cpu_avg: f32,
    pub cpu_count: usize,
    pub uptime: u64,
    pub cpus: Vec<f32>,
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
        .filter(|x| x.thread_kind() != Some(sysinfo::ThreadKind::Userland))
        .map(|x| {
            let cmd = match x.exe() {
                Some(s) if s.as_os_str().is_empty() => x.name().to_string_lossy().to_string(),
                Some(s) => s.to_string_lossy().to_string(),
                None => x.name().to_string_lossy().to_string(),
            };

            MyProcess {
                pid: x.pid().into(),
                parent: x.parent().map_or(0, |f| f.into()),
                name: x.name().to_string_lossy().to_string(),
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
