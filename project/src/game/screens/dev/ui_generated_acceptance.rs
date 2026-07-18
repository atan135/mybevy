use bevy::prelude::*;

use crate::framework::ui::{
    core::UiViewport,
    document::{
        UiDocumentPreviewCommand, UiDocumentRuntimeCommand, parse_approved_document_registration,
        target_profile_from_viewport,
    },
};

const DOCUMENT_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/generated_acceptance_fixture/document.v1.json"
);
const REGISTRATION_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/generated_acceptance_fixture/promotion.v1.json"
);

pub(super) fn setup_ui_generated_acceptance(
    viewport: Res<UiViewport>,
    mut preview_commands: MessageWriter<UiDocumentPreviewCommand>,
) {
    let registration = parse_approved_document_registration(REGISTRATION_SOURCE)
        .expect("promoted acceptance registration must match the closed adapter protocol");
    let preview = registration
        .to_preview_registration(
            DOCUMENT_SOURCE.to_owned(),
            target_profile_from_viewport(&viewport),
        )
        .expect("promoted acceptance document must pass the formal runtime adapter");
    preview_commands.write(UiDocumentPreviewCommand::Register(preview));
}

pub(super) fn cleanup_ui_generated_acceptance(
    mut preview_commands: MessageWriter<UiDocumentPreviewCommand>,
    mut runtime_commands: MessageWriter<UiDocumentRuntimeCommand>,
) {
    let registration = parse_approved_document_registration(REGISTRATION_SOURCE)
        .expect("promoted acceptance registration must remain valid");
    preview_commands.write(UiDocumentPreviewCommand::Unregister {
        document_id: registration.document_id().clone(),
        owner: registration.owner().to_owned(),
    });
    runtime_commands.write(UiDocumentRuntimeCommand::Close {
        owner: registration.owner().to_owned(),
        document_id: registration.document_id().clone(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::document::{
        UiDocument, UiDocumentInputMode, UiDocumentPlatform, UiSafeAreaClass, UiTargetProfile,
    };

    #[test]
    fn promoted_acceptance_page_is_valid_and_bound_to_the_game_owned_route() {
        let registration = parse_approved_document_registration(REGISTRATION_SOURCE).unwrap();
        assert_eq!(
            registration.document_id().as_str(),
            "generated.acceptance_fixture"
        );
        assert_eq!(registration.owner(), "generated_acceptance_approved");
        assert_eq!(registration.route(), "ui_generated_acceptance");
        assert_eq!(
            registration.source_path().as_str(),
            "ui/documents/approved/generated_acceptance_fixture/document.v1.json"
        );
        assert!(UiDocument::validate_json(DOCUMENT_SOURCE).report.valid);

        let target_profile = UiTargetProfile::new(
            390.0,
            844.0,
            UiSafeAreaClass::None,
            UiDocumentInputMode::MouseTouch,
            UiDocumentPlatform::Windows,
        )
        .unwrap();
        let preview = registration
            .to_preview_registration(DOCUMENT_SOURCE.to_owned(), target_profile)
            .unwrap();
        assert!(preview.open_on_register);
        assert!(!preview.watch);
        assert!(preview.host_bindings.is_empty());
    }
}
