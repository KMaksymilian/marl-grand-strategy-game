use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Wood,
    Stone,
    Grain,
    Labour, // work in progress
}

/// Resource groups of the gameplay specification (§2): food, building
/// materials and metals. Decrees and later systems select resources by group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceCategory {
    Food,
    BuildingMaterial,
    /// Ores, coal and metals (§2.1.3, §2.2.3).
    Metal,
}

impl ResourceType {
    /// The material catalogue used to validate complete storage orders.
    pub const MATERIALS: &'static [Self] = &[Self::Wood, Self::Stone, Self::Grain];

    /// Production tier (§2): 1 raw, 2 processed, 3 crafted. None for Labour.
    pub fn tier(self) -> Option<u8> {
        match self {
            Self::Wood | Self::Stone | Self::Grain => Some(1),
            Self::Labour => None,
        }
    }

    /// Specification group; None for Labour.
    pub fn category(self) -> Option<ResourceCategory> {
        match self {
            Self::Grain => Some(ResourceCategory::Food),
            Self::Wood | Self::Stone => Some(ResourceCategory::BuildingMaterial),
            Self::Labour => None,
        }
    }

    /// Mined ore or coal (§2.1.3). None of the current materials is one.
    pub fn is_ore(self) -> bool {
        self.tier() == Some(1) && self.category() == Some(ResourceCategory::Metal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materials_follow_the_specification_groups() {
        assert_eq!(ResourceType::Grain.category(), Some(ResourceCategory::Food));
        assert_eq!(
            ResourceType::Wood.category(),
            Some(ResourceCategory::BuildingMaterial)
        );
        assert_eq!(
            ResourceType::Stone.category(),
            Some(ResourceCategory::BuildingMaterial)
        );
        for material in ResourceType::MATERIALS {
            assert_eq!(material.tier(), Some(1));
            assert!(!material.is_ore());
        }
        assert_eq!(ResourceType::Labour.tier(), None);
        assert_eq!(ResourceType::Labour.category(), None);
    }
}
