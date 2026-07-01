use serde::{Deserialize, Serialize};

use super::FangyuanPrimitiveKind;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPrimitiveBlueprint {
    pub kind: FangyuanPrimitiveKind,
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub color: [f32; 4],
}

impl FangyuanPrimitiveBlueprint {
    pub const fn new(
        kind: FangyuanPrimitiveKind,
        position: [f32; 3],
        size: [f32; 3],
        color: [f32; 4],
    ) -> Self {
        Self {
            kind,
            position,
            size,
            color,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blueprint_primitive_accepts_only_expected_fields() {
        let blueprint: FangyuanPrimitiveBlueprint = serde_json::from_str(
            r#"{
                "kind": "cube",
                "position": [0.0, 0.5, 0.0],
                "size": [1.0, 1.0, 1.0],
                "color": [0.8, 0.6, 0.4, 1.0]
            }"#,
        )
        .unwrap();

        assert_eq!(blueprint.kind, FangyuanPrimitiveKind::Cube);
        assert_eq!(blueprint.position, [0.0, 0.5, 0.0]);
        assert_eq!(blueprint.size, [1.0, 1.0, 1.0]);
        assert_eq!(blueprint.color, [0.8, 0.6, 0.4, 1.0]);
    }

    #[test]
    fn blueprint_primitive_rejects_rotation_field() {
        let result = serde_json::from_str::<FangyuanPrimitiveBlueprint>(
            r#"{
                "kind": "sphere",
                "position": [0.0, 1.2, 0.0],
                "size": [0.8, 0.8, 0.8],
                "color": [0.9, 0.8, 0.7, 1.0],
                "rotation": [0.0, 0.0, 0.0]
            }"#,
        );

        assert!(result.is_err());
    }
}
