#[derive(Debug, Default, Clone, Copy)]
pub enum Priority {
    Low,
    #[default]
    Default,
    High,
}
