use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::sync::Arc;

/// Stable material identifier shared by scenarios and serialized game data.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId(Arc<str>);

impl ResourceId {
    /// Read a material identifier in the API's upper camel case format.
    pub fn new(name: &str) -> Option<Self> {
        if matches!(name, "Labour" | "Gold")
            || !name.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
            || !name.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return None;
        }
        Some(Self(Arc::from(name)))
    }

    /// Convert a canonical snake case content identifier to its stable API name.
    pub fn from_content_id(id: &str) -> Option<Self> {
        let mut name = String::with_capacity(id.len());
        for word in id.split('_') {
            let (first, rest) = word.as_bytes().split_first()?;
            if !first.is_ascii_lowercase()
                || !rest
                    .iter()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            {
                return None;
            }
            name.push(first.to_ascii_uppercase().into());
            name.push_str(&word[1..]);
        }
        Self::new(&name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ResourceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl Serialize for ResourceId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ResourceId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::new(&name)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid material ID {name:?}")))
    }
}

/// A material or renewable workforce capacity used by the allocation engine.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Material(ResourceId),
    Labour,
}

impl ResourceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Material(id) => id.as_str(),
            Self::Labour => "Labour",
        }
    }
}

impl Ord for ResourceType {
    fn cmp(&self, other: &Self) -> Ordering {
        // Bundled content keeps existing material tie breaks stable. Workforce
        // occupied the fourth slot in existing scenario and report ordering.
        // Catalogue loading uses hash maps and never invokes this comparator.
        let sort_key = |resource: &Self| {
            if resource == &Self::Labour {
                return 3;
            }
            super::ResourceCatalog::standard()
                .materials()
                .iter()
                .position(|material| material == resource)
                .map(|position| if position < 3 { position } else { position + 1 })
                .unwrap_or(usize::MAX)
        };
        sort_key(self)
            .cmp(&sort_key(other))
            .then_with(|| self.as_str().cmp(other.as_str()))
    }
}

impl PartialOrd for ResourceType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for ResourceType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ResourceType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        if name == "Labour" {
            return Ok(Self::Labour);
        }
        ResourceId::new(&name)
            .map(Self::Material)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid resource ID {name:?}")))
    }
}
