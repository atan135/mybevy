use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FangyuanPrimitiveKind {
    Cube,
    Sphere,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanPrimitive {
    pub kind: FangyuanPrimitiveKind,
    pub local_position: Vec3,
    pub scale: Vec3,
    pub color: Color,
}

impl FangyuanPrimitive {
    pub const fn new(
        kind: FangyuanPrimitiveKind,
        local_position: Vec3,
        scale: Vec3,
        color: Color,
    ) -> Self {
        Self {
            kind,
            local_position,
            scale,
            color,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FangyuanPrimitiveSet {
    primitives: Vec<FangyuanPrimitive>,
}

impl FangyuanPrimitiveSet {
    pub const fn new() -> Self {
        Self {
            primitives: Vec::new(),
        }
    }

    pub fn from_primitives(primitives: Vec<FangyuanPrimitive>) -> Self {
        Self { primitives }
    }

    pub fn len(&self) -> usize {
        self.primitives.len()
    }

    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty()
    }

    pub fn primitives(&self) -> &[FangyuanPrimitive] {
        &self.primitives
    }

    pub fn into_primitives(self) -> Vec<FangyuanPrimitive> {
        self.primitives
    }
}

impl From<Vec<FangyuanPrimitive>> for FangyuanPrimitiveSet {
    fn from(primitives: Vec<FangyuanPrimitive>) -> Self {
        Self::from_primitives(primitives)
    }
}

impl FromIterator<FangyuanPrimitive> for FangyuanPrimitiveSet {
    fn from_iter<T: IntoIterator<Item = FangyuanPrimitive>>(iter: T) -> Self {
        Self::from_primitives(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_kind_uses_lowercase_serde_names() {
        assert_eq!(
            serde_json::to_string(&FangyuanPrimitiveKind::Cube).unwrap(),
            r#""cube""#
        );
        assert_eq!(
            serde_json::from_str::<FangyuanPrimitiveKind>(r#""sphere""#).unwrap(),
            FangyuanPrimitiveKind::Sphere
        );
    }

    #[test]
    fn primitive_set_wraps_primitives_without_entity_identity() {
        let primitive = FangyuanPrimitive::new(
            FangyuanPrimitiveKind::Cube,
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::splat(1.0),
            Color::srgb(0.8, 0.6, 0.4),
        );

        let set = FangyuanPrimitiveSet::from_primitives(vec![primitive.clone()]);

        assert_eq!(set.len(), 1);
        assert_eq!(set.primitives(), &[primitive]);
    }
}
