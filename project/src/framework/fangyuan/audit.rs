/// Unified Fangyuan audit report shared by later blueprint, prefab, layout, and
/// runtime primitive-set checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanAuditReport {
    pub source_kind: FangyuanAuditSourceKind,
    pub source_path: Option<String>,
    pub status: FangyuanAuditStatus,
    pub summary: FangyuanAuditSummary,
    pub findings: Vec<FangyuanAuditFinding>,
    pub suggestions: Vec<FangyuanAuditSuggestion>,
}

impl FangyuanAuditReport {
    pub fn new(
        source_kind: FangyuanAuditSourceKind,
        source_path: impl Into<Option<String>>,
    ) -> Self {
        Self {
            source_kind,
            source_path: source_path.into(),
            status: FangyuanAuditStatus::Passed,
            summary: FangyuanAuditSummary::default(),
            findings: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn add_finding(&mut self, finding: FangyuanAuditFinding) {
        self.findings.push(finding);
        self.refresh_summary_and_status();
    }

    pub fn add_suggestion(&mut self, suggestion: FangyuanAuditSuggestion) {
        if !self.suggestions.contains(&suggestion) {
            self.suggestions.push(suggestion);
        }
    }

    pub fn sort_findings(&mut self) {
        self.findings.sort();
    }

    pub fn refresh_summary_and_status(&mut self) {
        self.summary = FangyuanAuditSummary::from_findings(&self.findings);
        self.status = FangyuanAuditStatus::from_summary(&self.summary);
    }
}

impl Default for FangyuanAuditReport {
    fn default() -> Self {
        Self::new(FangyuanAuditSourceKind::Unknown, None)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanAuditSummary {
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub authored_primitives: usize,
    pub generated_primitives: usize,
    pub skipped_primitives: usize,
    pub material_count: usize,
}

impl FangyuanAuditSummary {
    pub fn from_findings(findings: &[FangyuanAuditFinding]) -> Self {
        let mut summary = Self::default();
        for finding in findings {
            match finding.severity {
                FangyuanAuditSeverity::Error => summary.error_count += 1,
                FangyuanAuditSeverity::Warning => summary.warning_count += 1,
                FangyuanAuditSeverity::Info => summary.info_count += 1,
            }
        }
        summary
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FangyuanAuditStatus {
    #[default]
    Passed,
    PassedWithWarnings,
    Failed,
}

impl FangyuanAuditStatus {
    pub fn from_summary(summary: &FangyuanAuditSummary) -> Self {
        if summary.error_count > 0 {
            Self::Failed
        } else if summary.warning_count > 0 {
            Self::PassedWithWarnings
        } else {
            Self::Passed
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum FangyuanAuditSeverity {
    Error,
    Warning,
    #[default]
    Info,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum FangyuanAuditSourceKind {
    Blueprint,
    PrefabPalette,
    SceneLayout,
    RuntimePrimitiveSet,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanAuditFinding {
    pub severity: FangyuanAuditSeverity,
    pub code: String,
    pub field_path: Option<String>,
    pub reason: String,
    pub source_kind: FangyuanAuditSourceKind,
    pub source_path: Option<String>,
    pub primitive_index: Option<usize>,
    pub prefab_id: Option<String>,
    pub instance_id: Option<String>,
    pub instance_index: Option<usize>,
    pub prefab_primitive_index: Option<usize>,
}

impl FangyuanAuditFinding {
    pub fn new(
        severity: FangyuanAuditSeverity,
        code: impl Into<String>,
        reason: impl Into<String>,
        source_kind: FangyuanAuditSourceKind,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            reason: reason.into(),
            source_kind,
            ..Default::default()
        }
    }
}

impl Ord for FangyuanAuditFinding {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            self.severity,
            self.source_kind,
            self.field_path.as_deref(),
            self.code.as_str(),
            self.source_path.as_deref(),
            self.primitive_index,
            self.prefab_id.as_deref(),
            self.instance_id.as_deref(),
            self.instance_index,
            self.prefab_primitive_index,
            self.reason.as_str(),
        )
            .cmp(&(
                other.severity,
                other.source_kind,
                other.field_path.as_deref(),
                other.code.as_str(),
                other.source_path.as_deref(),
                other.primitive_index,
                other.prefab_id.as_deref(),
                other.instance_id.as_deref(),
                other.instance_index,
                other.prefab_primitive_index,
                other.reason.as_str(),
            ))
    }
}

impl PartialOrd for FangyuanAuditFinding {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanAuditSuggestion {
    pub action: FangyuanAuditSuggestionAction,
    pub field_path: Option<String>,
    pub reason: String,
    pub estimated_effect: Option<String>,
}

impl FangyuanAuditSuggestion {
    pub fn new(
        action: FangyuanAuditSuggestionAction,
        field_path: impl Into<Option<String>>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            action,
            field_path: field_path.into(),
            reason: reason.into(),
            estimated_effect: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanAuditSuggestionAction {
    ReducePrimitives,
    ShrinkBounds,
    LowerEmissive,
    RemoveAlpha,
    ReplaceMaterialProfile,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fangyuan_audit_report_defaults_to_passed_without_findings() {
        let report = FangyuanAuditReport::new(
            FangyuanAuditSourceKind::Blueprint,
            Some("fangyuan/avatars/minimal_player.ron".to_string()),
        );

        assert_eq!(report.status, FangyuanAuditStatus::Passed);
        assert_eq!(report.summary, FangyuanAuditSummary::default());
        assert_eq!(report.source_kind, FangyuanAuditSourceKind::Blueprint);
        assert_eq!(
            report.source_path.as_deref(),
            Some("fangyuan/avatars/minimal_player.ron")
        );
    }

    #[test]
    fn fangyuan_audit_status_passes_with_warnings_when_no_error_exists() {
        let mut report = FangyuanAuditReport::default();
        report.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Warning,
            "bounds.large",
            "bounds are larger than the mobile budget",
            FangyuanAuditSourceKind::SceneLayout,
        ));

        assert_eq!(report.status, FangyuanAuditStatus::PassedWithWarnings);
        assert_eq!(report.summary.warning_count, 1);
        assert_eq!(report.summary.error_count, 0);
    }

    #[test]
    fn fangyuan_audit_status_fails_when_error_exists() {
        let mut report = FangyuanAuditReport::default();
        report.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Warning,
            "material.alpha",
            "transparent material may be expensive",
            FangyuanAuditSourceKind::Blueprint,
        ));
        report.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            "primitive.count",
            "primitive count exceeds the hard limit",
            FangyuanAuditSourceKind::Blueprint,
        ));

        assert_eq!(report.status, FangyuanAuditStatus::Failed);
        assert_eq!(report.summary.error_count, 1);
        assert_eq!(report.summary.warning_count, 1);
    }

    #[test]
    fn fangyuan_audit_findings_sort_by_severity_and_stable_location_fields() {
        let mut report = FangyuanAuditReport::default();
        let mut warning = FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Warning,
            "material.alpha",
            "alpha is not preferred",
            FangyuanAuditSourceKind::Blueprint,
        );
        warning.field_path = Some("primitives[1].alpha".to_string());
        warning.primitive_index = Some(1);

        let mut info = FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Info,
            "material.profile",
            "default material profile used",
            FangyuanAuditSourceKind::PrefabPalette,
        );
        info.field_path = Some("prefabs[0].primitives[2].material_profile".to_string());
        info.prefab_id = Some("home_wall".to_string());
        info.prefab_primitive_index = Some(2);

        let mut error = FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            "bounds.exceeded",
            "primitive is outside bounds",
            FangyuanAuditSourceKind::SceneLayout,
        );
        error.field_path = Some("instances[0].position".to_string());
        error.source_path = Some("fangyuan/layouts/home_layout.ron".to_string());
        error.instance_id = Some("entry_wall".to_string());
        error.instance_index = Some(0);

        report.findings = vec![info, warning, error];
        report.sort_findings();

        assert_eq!(report.findings[0].severity, FangyuanAuditSeverity::Error);
        assert_eq!(report.findings[1].severity, FangyuanAuditSeverity::Warning);
        assert_eq!(report.findings[2].severity, FangyuanAuditSeverity::Info);
        assert_eq!(
            report.findings[0].field_path.as_deref(),
            Some("instances[0].position")
        );
        assert_eq!(
            report.findings[0].instance_id.as_deref(),
            Some("entry_wall")
        );
        assert_eq!(report.findings[1].primitive_index, Some(1));
        assert_eq!(report.findings[2].prefab_id.as_deref(), Some("home_wall"));
        assert_eq!(report.findings[2].prefab_primitive_index, Some(2));
    }

    #[test]
    fn fangyuan_audit_suggestions_are_deduplicated_by_action_field_and_reason() {
        let mut report = FangyuanAuditReport::default();
        let suggestion = FangyuanAuditSuggestion::new(
            FangyuanAuditSuggestionAction::ReducePrimitives,
            Some("primitives".to_string()),
            "primitive count exceeds the recommended budget",
        );

        report.add_suggestion(suggestion.clone());
        report.add_suggestion(suggestion);
        report.add_suggestion(FangyuanAuditSuggestion::new(
            FangyuanAuditSuggestionAction::LowerEmissive,
            Some("primitives[0].emissive".to_string()),
            "emissive intensity is above the target range",
        ));

        assert_eq!(report.suggestions.len(), 2);
        assert_eq!(
            report.suggestions[0].action,
            FangyuanAuditSuggestionAction::ReducePrimitives
        );
        assert_eq!(
            report.suggestions[0].field_path.as_deref(),
            Some("primitives")
        );
        assert_eq!(
            report.suggestions[1].action,
            FangyuanAuditSuggestionAction::LowerEmissive
        );
    }
}
