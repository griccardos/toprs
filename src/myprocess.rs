#[derive(Clone, Debug)]
pub struct MyProcess {
    pub pid: usize,
    pub parent: usize,
    pub name: String,
    pub command: String,
    pub memory: u64,
    pub cpu: f32,
    pub children_memory: u64,
    pub depth: usize,
}

impl MyProcess {
    pub fn total(&self) -> u64 {
        self.memory + self.children_memory
    }
}
