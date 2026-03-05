#[derive(Debug, Clone)]
pub struct MyNetwork {
    pub name: String,
    pub received: u64,
    pub sent: u64,
    pub received_per_sec: u64,
    pub sent_per_sec: u64,
}
