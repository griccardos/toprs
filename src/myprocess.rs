#[derive(Clone, Debug, Default)]
pub struct MyProcess {
    pub pid: usize,
    pub parent: usize,
    pub name: String,
    pub command: String,
    pub command_display: String, //for table view
    pub memory: u64,
    pub cpu: f32,
    pub disk: f64,
    pub children_memory: u64,
    pub depth: usize,
    pub run_time: u64,
}

impl MyProcess {
    pub fn total(&self) -> u64 {
        self.memory + self.children_memory
    }
}
