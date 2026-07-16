//! Closed, read-only contract for a promoted approved-page registration declaration.
//!
//! This adapter deliberately does not route a page or generate game code. A game screen must
//! explicitly choose when to register the resulting declarative page with its own route lifecycle.

use super::{
    UiDocument, UiDocumentId, UiDocumentLayer, UiDocumentPanel, UiDocumentPreviewRegistration,
    UiDocumentSourcePath, UiDocumentSourceRoot, UiPageState, UiTargetProfile,
};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, fmt, str::FromStr};

pub const UI_APPROVED_DOCUMENT_REGISTRATION_PROTOCOL_VERSION: u32 = 1;
const REGISTRATION_KIND: &str = "ui_document_promotion_registration";
const REGISTRATION_TEMPLATE_VERSION: u32 = 1;
const REQUIRED_AUDIT_PROFILES: [&str; 4] = [
    "phone-small",
    "phone-portrait",
    "tablet-portrait",
    "tablet-landscape",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiApprovedDocumentRegistration {
    document_id: UiDocumentId,
    source_path: UiDocumentSourcePath,
    owner: String,
    route: String,
    page_state: UiPageState,
    audit_profiles: Vec<String>,
}

impl UiApprovedDocumentRegistration {
    pub fn document_id(&self) -> &UiDocumentId {
        &self.document_id
    }

    pub fn source_path(&self) -> &UiDocumentSourcePath {
        &self.source_path
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// A review-only route label. The adapter never dispatches a game route from this string.
    pub fn route(&self) -> &str {
        &self.route
    }

    pub fn page_state(&self) -> &UiPageState {
        &self.page_state
    }

    pub fn audit_profiles(&self) -> &[String] {
        &self.audit_profiles
    }

    /// Converts a reviewed declaration into the existing formal preview/runtime registration.
    /// A game-owned route adapter must call this explicitly during its own lifecycle.
    pub fn to_preview_registration(
        &self,
        source_json: String,
        target_profile: UiTargetProfile,
    ) -> Result<UiDocumentPreviewRegistration, UiApprovedDocumentRegistrationError> {
        let validation = UiDocument::validate_json(&source_json);
        let document = validation.validated().ok_or_else(|| {
            UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_DOCUMENT_INVALID",
                "approved registration source does not pass formal UiDocument validation",
            )
        })?;
        if document.document().document_id != self.document_id {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_DOCUMENT_ID_MISMATCH",
                "approved registration document_id differs from its source document",
            ));
        }
        reject_business_fields(&serde_json::from_str::<Value>(&source_json).map_err(|_| {
            UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_DOCUMENT_INVALID",
                "approved registration source cannot be decoded as JSON",
            )
        })?)?;
        Ok(UiDocumentPreviewRegistration {
            document_id: self.document_id.clone(),
            owner: self.owner.clone(),
            source_path: self.source_path.clone(),
            source_json,
            panel: UiDocumentPanel::Page,
            layer: UiDocumentLayer::Page,
            target_profile,
            page_state: self.page_state.clone(),
            owner_alive: true,
            host_bindings: BTreeMap::new(),
            watch: false,
            open_on_register: true,
            audit_profiles: self.audit_profiles.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiApprovedDocumentRegistrationError {
    code: &'static str,
    message: String,
}

impl UiApprovedDocumentRegistrationError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for UiApprovedDocumentRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for UiApprovedDocumentRegistrationError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationFile {
    protocol_version: u32,
    kind: String,
    template_version: u32,
    document_id: String,
    source: RegistrationSource,
    owner: String,
    route: String,
    panel: String,
    layer: String,
    page_state: String,
    audit_profiles: Vec<String>,
    i18n_keys: Vec<String>,
    theme_tokens: Vec<String>,
    action_or_binding_registration: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationSource {
    root: String,
    relative_path: String,
}

/// Parses only the closed promotion declaration emitted by the development tool.
pub fn parse_approved_document_registration(
    source: &str,
) -> Result<UiApprovedDocumentRegistration, UiApprovedDocumentRegistrationError> {
    let file: RegistrationFile = serde_json::from_str(source).map_err(|_| {
        UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_SCHEMA_INVALID",
            "approved registration must match the closed JSON schema",
        )
    })?;
    if file.protocol_version != UI_APPROVED_DOCUMENT_REGISTRATION_PROTOCOL_VERSION
        || file.kind != REGISTRATION_KIND
        || file.template_version != REGISTRATION_TEMPLATE_VERSION
        || file.source.root != "approved"
        || file.panel != "page"
        || file.layer != "page"
        || !file.i18n_keys.is_empty()
        || !file.theme_tokens.is_empty()
        || !file.action_or_binding_registration.is_empty()
    {
        return Err(UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_CLOSED_FIELD_REJECTED",
            "approved registration contains an unsupported protocol field or business registration",
        ));
    }
    let document_id = UiDocumentId::from_str(&file.document_id).map_err(|_| {
        UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_DOCUMENT_ID_INVALID",
            "approved registration document_id is invalid",
        )
    })?;
    let source_path =
        UiDocumentSourcePath::new(UiDocumentSourceRoot::Approved, file.source.relative_path)
            .map_err(|_| {
                UiApprovedDocumentRegistrationError::new(
                    "UI_APPROVED_REGISTRATION_SOURCE_INVALID",
                    "approved registration source path is invalid",
                )
            })?;
    if !safe_registration_label(&file.owner) || !safe_registration_label(&file.route) {
        return Err(UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_OWNER_ROUTE_INVALID",
            "approved registration owner or route is invalid",
        ));
    }
    let page_state = UiPageState::from_str(&file.page_state).map_err(|_| {
        UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_PAGE_STATE_INVALID",
            "approved registration page state is invalid",
        )
    })?;
    if page_state != UiPageState::initial() || normalized_profiles(&file.audit_profiles).is_none() {
        return Err(UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_AUDIT_INVALID",
            "approved registration page state or audit profiles differ from the closed template",
        ));
    }
    Ok(UiApprovedDocumentRegistration {
        document_id,
        source_path,
        owner: file.owner,
        route: file.route,
        page_state,
        audit_profiles: REQUIRED_AUDIT_PROFILES.map(str::to_owned).to_vec(),
    })
}

fn normalized_profiles(values: &[String]) -> Option<Vec<String>> {
    if values.len() != REQUIRED_AUDIT_PROFILES.len() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort();
    values.dedup();
    let mut expected = REQUIRED_AUDIT_PROFILES.map(str::to_owned).to_vec();
    expected.sort();
    (values == expected).then_some(expected)
}

fn safe_registration_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_uppercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn reject_business_fields(value: &Value) -> Result<(), UiApprovedDocumentRegistrationError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if matches!(
                    key.as_str(),
                    "action" | "on_click" | "binding_path" | "i18n_key"
                ) || (key == "bindings"
                    && child.as_object().is_some_and(|value| !value.is_empty()))
                {
                    return Err(UiApprovedDocumentRegistrationError::new(
                        "UI_APPROVED_REGISTRATION_BUSINESS_FIELD_REJECTED",
                        "approved registration source contains an action, binding, or i18n field",
                    ));
                }
                reject_business_fields(child)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                reject_business_fields(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::{
        core::{UiMetrics, focus::UiFocusState},
        style::{UiFontAssets, UiTheme},
    };
    use bevy::{
        asset::RenderAssetUsages,
        prelude::{App, Assets, Image},
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };

    const REGISTRATION_JSON: &str = r#"{
      "protocol_version": 1,
      "kind": "ui_document_promotion_registration",
      "template_version": 1,
      "document_id": "promotion.runtime",
      "source": { "root": "approved", "relative_path": "promotion_runtime/document.v1.json" },
      "owner": "promotion_owner",
      "route": "promotion_route",
      "panel": "page",
      "layer": "page",
      "page_state": "initial",
      "audit_profiles": ["phone-small", "phone-portrait", "tablet-portrait", "tablet-landscape"],
      "i18n_keys": [],
      "theme_tokens": [],
      "action_or_binding_registration": []
    }"#;

    fn target_profile() -> UiTargetProfile {
        UiTargetProfile::new(
            390.0,
            844.0,
            super::super::UiSafeAreaClass::None,
            super::super::UiDocumentInputMode::MouseTouch,
            super::super::UiDocumentPlatform::Windows,
        )
        .unwrap()
    }

    fn approved_image_document() -> String {
        serde_json::json!({
            "schema_version": 1,
            "document_id": "promotion.runtime",
            "assets": {
                "promotion_image": {
                    "kind": "icon",
                    "source": {
                        "kind": "packaged",
                        "path": "ui/documents/approved/promotion_runtime/assets/promotion.png"
                    }
                }
            },
            "root": {
                "type": "icon",
                "id": "promotion.image",
                "asset": "promotion_image"
            }
        })
        .to_string()
    }

    fn binding_document() -> String {
        serde_json::json!({
            "schema_version": 1,
            "document_id": "promotion.runtime",
            "bindings": {
                "state.title": {
                    "scope": "local",
                    "value_type": { "kind": "string" },
                    "default": { "kind": "string", "value": "Approved" },
                    "missing": "use_default"
                }
            },
            "root": {
                "type": "text",
                "id": "promotion.title",
                "content": { "binding_path": "state.title", "fallback": "Approved" }
            }
        })
        .to_string()
    }

    fn test_image_handle(app: &mut App) -> bevy::prelude::Handle<Image> {
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new_fill(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[255, 255, 255, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            ))
    }

    #[test]
    fn approved_document_registration_parses_the_closed_promotion_template() {
        let registration = parse_approved_document_registration(REGISTRATION_JSON).unwrap();
        assert_eq!(registration.document_id().as_str(), "promotion.runtime");
        assert_eq!(
            registration.source_path().as_str(),
            "ui/documents/approved/promotion_runtime/document.v1.json"
        );
        assert_eq!(registration.owner(), "promotion_owner");
        assert_eq!(registration.route(), "promotion_route");
        assert_eq!(registration.page_state(), &UiPageState::initial());
        assert_eq!(
            registration.audit_profiles(),
            [
                "phone-small",
                "phone-portrait",
                "tablet-portrait",
                "tablet-landscape"
            ]
        );
    }

    #[test]
    fn approved_document_registration_rejects_business_registration_fields() {
        let mut registration: Value = serde_json::from_str(REGISTRATION_JSON).unwrap();
        registration["action_or_binding_registration"] = serde_json::json!(["route.execute"]);
        let error = parse_approved_document_registration(&registration.to_string()).unwrap_err();
        assert_eq!(
            error.code(),
            "UI_APPROVED_REGISTRATION_CLOSED_FIELD_REJECTED"
        );

        let mut registration: Value = serde_json::from_str(REGISTRATION_JSON).unwrap();
        registration["audit_profiles"] = serde_json::json!([
            "phone-small",
            "phone-small",
            "phone-portrait",
            "tablet-portrait",
            "tablet-landscape"
        ]);
        let error = parse_approved_document_registration(&registration.to_string()).unwrap_err();
        assert_eq!(error.code(), "UI_APPROVED_REGISTRATION_AUDIT_INVALID");
    }

    #[test]
    fn approved_document_registration_rejects_document_business_fields_before_conversion() {
        let registration = parse_approved_document_registration(REGISTRATION_JSON).unwrap();
        let error = registration
            .to_preview_registration(binding_document(), target_profile())
            .unwrap_err();
        assert_eq!(
            error.code(),
            "UI_APPROVED_REGISTRATION_BUSINESS_FIELD_REJECTED"
        );

        for source in [
            serde_json::json!({ "on_click": { "action": "promotion.open" } }),
            serde_json::json!({ "action": "promotion.open" }),
            serde_json::json!({ "binding_path": "state.title" }),
            serde_json::json!({ "i18n_key": "promotion.title" }),
        ] {
            let error = reject_business_fields(&source).unwrap_err();
            assert_eq!(
                error.code(),
                "UI_APPROVED_REGISTRATION_BUSINESS_FIELD_REJECTED"
            );
        }
    }

    #[test]
    fn approved_document_registration_requires_explicit_lifecycle_registration() {
        let registration = parse_approved_document_registration(REGISTRATION_JSON).unwrap();
        let source_json = approved_image_document();
        let document_id = registration.document_id().clone();
        let preview_registration = registration
            .to_preview_registration(source_json, target_profile())
            .unwrap();
        assert_eq!(preview_registration.page_state, UiPageState::initial());
        assert_eq!(registration.route(), "promotion_route");

        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        app.insert_resource(UiTheme::default());
        app.insert_resource(UiMetrics::default());
        app.insert_resource(UiFontAssets::test_registry());
        app.init_resource::<UiFocusState>();
        app.add_plugins((
            super::super::UiDocumentRuntimePlugin,
            super::super::UiDocumentPreviewPlugin,
        ));
        let image = test_image_handle(&mut app);
        app.world_mut()
            .resource_mut::<super::super::UiDocumentAssetPreflightOverrides>()
            .set(
                document_id.clone(),
                super::super::UiAssetId::from_str("promotion_image").unwrap(),
                super::super::UiDocumentAssetPreflightStatus::Ready {
                    asset: super::super::UiDocumentResolvedAsset::Image(image),
                },
            );
        app.world_mut()
            .write_message(super::super::UiDocumentPreviewCommand::Register(
                preview_registration,
            ));
        app.update();
        app.update();

        assert!(
            app.world()
                .resource::<super::super::UiDocumentRuntime>()
                .active_instance("promotion_owner", &document_id)
                .is_some()
        );
        let recipe = app
            .world()
            .resource::<super::super::UiDocumentAuditRecipeRegistry>()
            .entry(&document_id, "promotion_owner")
            .unwrap();
        assert_eq!(recipe.screen, "document_promotion_runtime");
        assert_eq!(
            recipe.source_path,
            "ui/documents/approved/promotion_runtime/document.v1.json"
        );
        assert_eq!(
            recipe.profiles,
            [
                "phone-portrait",
                "phone-small",
                "tablet-landscape",
                "tablet-portrait"
            ]
        );

        app.world_mut()
            .write_message(super::super::UiDocumentPreviewCommand::Unregister {
                document_id: document_id.clone(),
                owner: "promotion_owner".to_owned(),
            });
        app.world_mut()
            .write_message(super::super::UiDocumentRuntimeCommand::Close {
                owner: "promotion_owner".to_owned(),
                document_id: document_id.clone(),
            });
        app.update();
        app.update();

        assert!(
            app.world()
                .resource::<super::super::UiDocumentAuditRecipeRegistry>()
                .entry(&document_id, "promotion_owner")
                .is_none()
        );
        assert!(
            app.world()
                .resource::<super::super::UiDocumentRuntime>()
                .active_instance("promotion_owner", &document_id)
                .is_none()
        );
    }
}
