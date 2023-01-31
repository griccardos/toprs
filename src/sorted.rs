use std::{cmp::Reverse, collections::HashSet};

use crate::{helpers::nice_size, myprocess::MyProcess};

#[derive(PartialEq, Debug)]
pub enum SortType {
    Ascending,
    Descending,
    None,
}
pub struct SortedProcesses {
    pub sort_col: usize,
    pub sort_type: SortType,
    pub hidezeros: bool,

    procs: Vec<MyProcess>,
}

impl SortedProcesses {
    pub fn new() -> Self {
        Self {
            sort_col: 0,
            sort_type: SortType::None,
            procs: vec![],
            hidezeros: true,
        }
    }

    pub fn update(&mut self, procs: &[MyProcess]) {
        self.procs = procs.to_vec();
        self.sort();
    }
    pub fn procs(&self) -> Vec<Vec<String>> {
        self.procs
            .iter()
            .filter(|f| if self.hidezeros { f.memory != 0 } else { true })
            .map(|f| {
                vec![
                    f.command.clone(),
                    f.name.clone(),
                    f.pid.to_string(),
                    nice_size(f.memory),
                    nice_size(f.children_memory),
                    nice_size(f.total()),
                    format!("{:.1}%", f.cpu),
                ]
            })
            .collect()
    }

    fn sort(&mut self) {
        match self.sort_col {
            0 => self
                .procs
                .sort_by(|a, b| b.command.to_lowercase().cmp(&a.command.to_lowercase())),
            1 => self
                .procs
                .sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase())),
            2 => self.procs.sort_by_key(|a| Reverse(a.pid)),
            3 => self.procs.sort_by_key(|a| Reverse(a.memory)),
            4 => self
                .procs
                .sort_by(|a, b| b.children_memory.cmp(&a.children_memory)),
            5 => self.procs.sort_by_key(|a| Reverse(a.total())),
            6 => self
                .procs
                .sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap()),

            _ => unreachable!(),
        }
        if self.sort_type == SortType::Ascending {
            self.procs.reverse();
        }

        if self.sort_type == SortType::None {
            Self::sort_by_command_with_tree(&mut self.procs);
        }
    }

    fn sort_by_command_with_tree(procs: &mut Vec<MyProcess>) {
        procs.sort_by_key(|a| std::cmp::Reverse(a.total()));
        //get procs, and list of their children, sort by memtotal
        //this should put tree in order
        let ordered = children_of(0, procs, 0);

        //difficult part is determining look of the tree:
        // if it has a sibling before, or parent it needs up
        // if it has a sibling after, it needs down
        let mut result = vec![];

        let mut later_siblings = HashSet::new();
        for i in 0..ordered.len() - 1 {
            let mut child = procs
                .iter()
                .find(|f| f.pid == ordered[i].0)
                .unwrap()
                .clone();

            let mut has = false;
            for j in i + 1..ordered.len() - 1 {
                //if has sibling
                if ordered[j].1 == ordered[i].1 {
                    has = true;
                    break;
                }
                //if lower level than parent, then no sibling, we can already break
                if ordered[j].1 < ordered[i].1 {
                    break;
                }
            }
            if has {
                later_siblings.insert(ordered[i].1);
            } else {
                later_siblings.remove(&ordered[i].1);
            }

            let mut sym = String::new();
            for j in 1..=ordered[i].1 {
                if ordered[i].1 == 0 {
                } else if later_siblings.contains(&j) {
                    if j == ordered[i].1 {
                        sym.push('├');
                    } else {
                        sym.push('│');
                    }
                } else if !later_siblings.contains(&i) && j == ordered[i].1 {
                    sym.push('└');
                } else {
                    sym.push(' ');
                }
            }

            child.command = format!("{sym}{}", child.command);
            result.push(child);
        }
        *procs = result;
    }

    pub fn sort_cycle(&mut self) {
        match (self.sort_col, &self.sort_type) {
            (0, SortType::None) => self.sort_type = SortType::Ascending,
            (0, SortType::Ascending) => self.sort_type = SortType::Descending,
            (0, SortType::Descending) => self.sort_type = SortType::None,
            (_, SortType::None) => self.sort_type = SortType::Descending,
            (_, SortType::Ascending) => self.sort_type = SortType::Descending,
            (_, SortType::Descending) => self.sort_type = SortType::Ascending,
        }
    }
}

///this returns the children of given, their level
fn children_of(pid: usize, procs: &Vec<MyProcess>, level: usize) -> Vec<(usize, usize)> {
    let mut vec = vec![];
    for p in procs.iter().filter(|f| f.parent == pid) {
        vec.push((p.pid, level));
        vec.extend(children_of(p.pid, procs, level + 1))
    }
    vec
}
