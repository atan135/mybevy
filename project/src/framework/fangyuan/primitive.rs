use bevy::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, de};

/// Runtime primitive kind compiled from blueprint data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FangyuanPrimitiveKind {
    Cube,
    Sphere,
}

impl Default for FangyuanPrimitiveKind {
    fn default() -> Self {
        Self::Cube
    }
}

impl FangyuanPrimitiveKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cube => "cube",
            Self::Sphere => "sphere",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "cube" => Some(Self::Cube),
            "sphere" => Some(Self::Sphere),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for FangyuanPrimitiveKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| de::Error::unknown_variant(&value, &["cube", "sphere"]))
    }
}

/// Semantic role of a Fangyuan primitive.
///
/// Roles are metadata for review, budgets, and later LOD decisions. They do not
/// define gameplay entity boundaries or rendering behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FangyuanPrimitiveRole {
    Structure,
    Core,
    Boundary,
    Warning,
    Trail,
    Impact,
    Decoration,
    Socket,
    Archive,
}

impl Default for FangyuanPrimitiveRole {
    fn default() -> Self {
        Self::Structure
    }
}

impl FangyuanPrimitiveRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Structure => "structure",
            Self::Core => "core",
            Self::Boundary => "boundary",
            Self::Warning => "warning",
            Self::Trail => "trail",
            Self::Impact => "impact",
            Self::Decoration => "decoration",
            Self::Socket => "socket",
            Self::Archive => "archive",
        }
    }

    pub const fn default_for_kind(kind: FangyuanPrimitiveKind) -> Self {
        match kind {
            FangyuanPrimitiveKind::Cube => Self::Structure,
            FangyuanPrimitiveKind::Sphere => Self::Core,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "structure" => Some(Self::Structure),
            "core" => Some(Self::Core),
            "boundary" => Some(Self::Boundary),
            "warning" => Some(Self::Warning),
            "trail" => Some(Self::Trail),
            "impact" => Some(Self::Impact),
            "decoration" => Some(Self::Decoration),
            "socket" => Some(Self::Socket),
            "archive" => Some(Self::Archive),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for FangyuanPrimitiveRole {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const EXPECTED: &[&str] = &[
            "structure",
            "core",
            "boundary",
            "warning",
            "trail",
            "impact",
            "decoration",
            "socket",
            "archive",
        ];

        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| de::Error::unknown_variant(&value, EXPECTED))
    }
}

/// Runtime primitive data compiled from a blueprint primitive.
///
/// Rendering features should translate this data into their own render instance
/// entities instead of treating blueprint records as renderable objects.
#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanPrimitive {
    /// Primitive geometry kind. Runtime primitives currently support only cube
    /// and sphere.
    pub kind: FangyuanPrimitiveKind,
    /// Primitive-local offset under the logical Entity root node.
    pub local_position: Vec3,
    /// Primitive-local scale. This does not encode rotation or facing.
    pub scale: Vec3,
    /// Primitive display color.
    pub color: Color,
    /// Primitive semantic role. This is metadata only and does not change the
    /// gameplay entity boundary.
    pub role: FangyuanPrimitiveRole,
}

impl FangyuanPrimitive {
    pub const fn new(
        kind: FangyuanPrimitiveKind,
        local_position: Vec3,
        scale: Vec3,
        color: Color,
    ) -> Self {
        Self::with_role(
            kind,
            local_position,
            scale,
            color,
            FangyuanPrimitiveRole::default_for_kind(kind),
        )
    }

    pub const fn with_role(
        kind: FangyuanPrimitiveKind,
        local_position: Vec3,
        scale: Vec3,
        color: Color,
        role: FangyuanPrimitiveRole,
    ) -> Self {
        Self {
            kind,
            local_position,
            scale,
            color,
            role,
        }
    }

    pub const fn kind(&self) -> FangyuanPrimitiveKind {
        self.kind
    }

    pub const fn local_position(&self) -> Vec3 {
        self.local_position
    }

    pub const fn scale(&self) -> Vec3 {
        self.scale
    }

    pub const fn color(&self) -> Color {
        self.color
    }

    pub const fn role(&self) -> FangyuanPrimitiveRole {
        self.role
    }
}

impl Default for FangyuanPrimitive {
    fn default() -> Self {
        Self::new(
            FangyuanPrimitiveKind::default(),
            Vec3::ZERO,
            Vec3::ONE,
            Color::WHITE,
        )
    }
}

/// Runtime primitive collection stored on the gameplay entity.
#[derive(Component, Clone, Debug, Default, PartialEq)]
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
    fn primitive_role_covers_expected_lowercase_serde_names() {
        let cases = [
            (FangyuanPrimitiveRole::Structure, "structure"),
            (FangyuanPrimitiveRole::Core, "core"),
            (FangyuanPrimitiveRole::Boundary, "boundary"),
            (FangyuanPrimitiveRole::Warning, "warning"),
            (FangyuanPrimitiveRole::Trail, "trail"),
            (FangyuanPrimitiveRole::Impact, "impact"),
            (FangyuanPrimitiveRole::Decoration, "decoration"),
            (FangyuanPrimitiveRole::Socket, "socket"),
            (FangyuanPrimitiveRole::Archive, "archive"),
        ];

        for (role, name) in cases {
            assert_eq!(role.as_str(), name);
            assert_eq!(
                serde_json::to_string(&role).unwrap(),
                format!(r#""{name}""#)
            );
            assert_eq!(
                serde_json::from_str::<FangyuanPrimitiveRole>(&format!(r#""{name}""#)).unwrap(),
                role
            );
        }
    }

    #[test]
    fn primitive_role_rejects_unknown_serde_name() {
        assert!(serde_json::from_str::<FangyuanPrimitiveRole>(r#""equipment""#).is_err());
    }

    #[test]
    fn primitive_kind_default_is_cube() {
        assert_eq!(
            FangyuanPrimitiveKind::default(),
            FangyuanPrimitiveKind::Cube
        );
    }

    #[test]
    fn primitive_role_default_is_structure() {
        assert_eq!(
            FangyuanPrimitiveRole::default(),
            FangyuanPrimitiveRole::Structure
        );
    }

    #[test]
    fn primitive_role_default_for_kind_marks_sphere_as_core() {
        assert_eq!(
            FangyuanPrimitiveRole::default_for_kind(FangyuanPrimitiveKind::Cube),
            FangyuanPrimitiveRole::Structure
        );
        assert_eq!(
            FangyuanPrimitiveRole::default_for_kind(FangyuanPrimitiveKind::Sphere),
            FangyuanPrimitiveRole::Core
        );
    }

    #[test]
    fn primitive_constructor_stores_runtime_fields() {
        let local_position = Vec3::new(0.25, 1.5, -0.75);
        let scale = Vec3::new(0.5, 1.25, 2.0);
        let color = Color::srgba(0.2, 0.4, 0.6, 0.8);

        let primitive =
            FangyuanPrimitive::new(FangyuanPrimitiveKind::Sphere, local_position, scale, color);

        assert_eq!(primitive.kind(), FangyuanPrimitiveKind::Sphere);
        assert_eq!(primitive.local_position(), local_position);
        assert_eq!(primitive.scale(), scale);
        assert_eq!(primitive.color(), color);
        assert_eq!(primitive.role(), FangyuanPrimitiveRole::Core);
    }

    #[test]
    fn primitive_constructor_can_store_explicit_role() {
        let primitive = FangyuanPrimitive::with_role(
            FangyuanPrimitiveKind::Cube,
            Vec3::ZERO,
            Vec3::ONE,
            Color::WHITE,
            FangyuanPrimitiveRole::Warning,
        );

        assert_eq!(primitive.role(), FangyuanPrimitiveRole::Warning);
    }

    #[test]
    fn primitive_default_is_legal_identity_cube() {
        let primitive = FangyuanPrimitive::default();

        assert_eq!(primitive.kind(), FangyuanPrimitiveKind::Cube);
        assert_eq!(primitive.local_position(), Vec3::ZERO);
        assert_eq!(primitive.scale(), Vec3::ONE);
        assert_eq!(primitive.color().to_srgba(), Color::WHITE.to_srgba());
        assert_eq!(primitive.role(), FangyuanPrimitiveRole::Structure);
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

    #[test]
    fn primitive_set_is_framework_component_api() {
        let mut app = App::new();
        let entity = app.world_mut().spawn(FangyuanPrimitiveSet::new()).id();

        assert!(
            app.world()
                .entity(entity)
                .contains::<FangyuanPrimitiveSet>()
        );

        let mut primitive_sets = app.world_mut().query::<&FangyuanPrimitiveSet>();
        assert!(primitive_sets.single(app.world()).unwrap().is_empty());
    }
}
