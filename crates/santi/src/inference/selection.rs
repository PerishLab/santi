use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Selection<T> {
    Scenario(T),
    Provider(T),
}

impl<T> Selection<T> {
    pub fn value(&self) -> &T {
        match self {
            Self::Scenario(value) | Self::Provider(value) => value,
        }
    }

    pub fn with<U>(&self, value: U) -> Selection<U> {
        match self {
            Self::Scenario(_) => Selection::Scenario(value),
            Self::Provider(_) => Selection::Provider(value),
        }
    }
}

impl Serialize for super::Effective {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.check().map_err(serde::ser::Error::custom)?;
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for super::Effective {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let snapshot = Self::deserialize(deserializer)?;
        snapshot.check().map_err(serde::de::Error::custom)?;
        Ok(snapshot)
    }
}

impl super::Effective {
    pub fn check(&self) -> Result<(), &'static str> {
        if let Selection::Scenario(value) = &self.selection
            && !super::path(value)
        {
            return Err("invalid inference scenario path");
        }
        if self.selection.with(*self.source.selection.value()) != self.source.selection {
            return Err("inference selection and source use different keys");
        }
        Ok(())
    }
}
