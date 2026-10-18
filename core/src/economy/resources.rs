#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceType {
    Wood,
    Stone,
    Grain,
    Labour, // work in progress
}

impl ResourceType {
    /// The material catalogue used to validate complete storage orders.
    pub const MATERIALS: &'static [Self] = &[Self::Wood, Self::Stone, Self::Grain];
}
