use std::{collections::BTreeMap, str::FromStr};

use bevy::prelude::*;

use crate::{
    framework::ui::{
        core::UiViewport,
        document::{
            UiDocumentId, UiDocumentLayer, UiDocumentPanel, UiDocumentPreviewCommand,
            UiDocumentPreviewRegistration, UiDocumentRuntimeCommand, UiDocumentSourcePath,
            UiDocumentSourceRoot, UiPageState, target_profile_from_viewport,
        },
    },
    game::{navigation::UI_DOCUMENT_GALLERY_DOCUMENT, ui_ids::OWNER_UI_DOCUMENT_GALLERY},
};

const GALLERY_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/gallery/declarative_gallery.v1.json");

pub(super) fn setup_ui_document_gallery(
    viewport: Res<UiViewport>,
    mut commands: MessageWriter<UiDocumentPreviewCommand>,
) {
    commands.write(UiDocumentPreviewCommand::Register(
        UiDocumentPreviewRegistration {
            document_id: UiDocumentId::from_str(UI_DOCUMENT_GALLERY_DOCUMENT)
                .expect("Gallery document ID is static and valid"),
            owner: OWNER_UI_DOCUMENT_GALLERY.as_str().to_owned(),
            source_path: UiDocumentSourcePath::new(
                UiDocumentSourceRoot::Approved,
                "gallery/declarative_gallery.v1.json",
            )
            .expect("Gallery source is a safe approved path"),
            source_json: GALLERY_SOURCE.to_owned(),
            panel: UiDocumentPanel::Page,
            layer: UiDocumentLayer::Page,
            target_profile: target_profile_from_viewport(&viewport),
            page_state: UiPageState::initial(),
            owner_alive: true,
            host_bindings: BTreeMap::new(),
            watch: true,
            open_on_register: true,
            audit_profiles: vec![
                "phone-small".to_owned(),
                "phone-portrait".to_owned(),
                "tablet-portrait".to_owned(),
                "tablet-landscape".to_owned(),
            ],
        },
    ));
}

pub(super) fn cleanup_ui_document_gallery(
    mut preview_commands: MessageWriter<UiDocumentPreviewCommand>,
    mut runtime_commands: MessageWriter<UiDocumentRuntimeCommand>,
) {
    let document_id = UiDocumentId::from_str(UI_DOCUMENT_GALLERY_DOCUMENT)
        .expect("Gallery document ID is static and valid");
    preview_commands.write(UiDocumentPreviewCommand::Unregister {
        document_id: document_id.clone(),
        owner: OWNER_UI_DOCUMENT_GALLERY.as_str().to_owned(),
    });
    runtime_commands.write(UiDocumentRuntimeCommand::Close {
        owner: OWNER_UI_DOCUMENT_GALLERY.as_str().to_owned(),
        document_id,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::document::{UiDocument, UiNode};

    #[test]
    fn declarative_gallery_is_a_valid_complete_runtime_document() {
        let validation = UiDocument::validate_json(GALLERY_SOURCE);
        assert!(
            validation.report.valid,
            "{:?}",
            validation.report.diagnostics
        );
        let document = validation.validated().unwrap().document();
        assert_eq!(document.document_id.as_str(), UI_DOCUMENT_GALLERY_DOCUMENT);
        assert!(!document.assets.is_empty());
        assert!(!document.bindings.is_empty());
        assert!(!document.responsive.is_empty());

        let mut kinds = Vec::new();
        fn visit(node: &UiNode, kinds: &mut Vec<&'static str>) {
            kinds.push(match node {
                UiNode::Container { .. } => "container",
                UiNode::Text { .. } => "text",
                UiNode::Image { .. } => "image",
                UiNode::Button { .. } => "button",
                UiNode::TextInput { .. } => "text_input",
                UiNode::Checkbox { .. } => "checkbox",
                UiNode::Toggle { .. } => "toggle",
                UiNode::Segmented { .. } => "segmented",
                UiNode::Slider { .. } => "slider",
                UiNode::Stepper { .. } => "stepper",
                UiNode::Progress { .. } => "progress",
                UiNode::Tab { .. } => "tab",
                UiNode::Select { .. } => "select",
                UiNode::Badge { .. } => "badge",
                UiNode::Tooltip { .. } => "tooltip",
                UiNode::Scroll { .. } => "scroll",
                UiNode::Icon { .. } => "icon",
                UiNode::Spacer { .. } => "spacer",
                UiNode::Modal { .. } => "modal",
                UiNode::ImageButton { .. } => "image_button",
            });
            for child in node.children() {
                visit(child, kinds);
            }
        }
        visit(&document.root, &mut kinds);
        for expected in [
            "text",
            "image",
            "button",
            "text_input",
            "checkbox",
            "toggle",
            "segmented",
            "slider",
            "stepper",
            "progress",
            "tab",
            "select",
            "badge",
            "tooltip",
        ] {
            assert!(kinds.contains(&expected), "missing {expected}");
        }
    }
}
