fn main() {
    match project::framework::ui::document::UiDocumentStandalonePreviewOptions::parse_env_args()
        .and_then(project::framework::ui::document::run_ui_document_standalone_preview)
    {
        Ok(()) => {}
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::to_string(&error).unwrap_or_else(|_| error.to_string())
            );
            std::process::exit(1);
        }
    }
}
