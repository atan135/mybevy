use bevy::prelude::*;
use serde::Serialize;

use crate::framework::ui::core::UiWidthClass;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum UiVisualCapability {
    Layout,
    Typography,
    Image,
    Slice,
    Icon,
    Surface,
    Border,
    Shadow,
    Gradient,
    Animation,
    ControlState,
}

impl UiVisualCapability {
    #[allow(dead_code)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Typography => "typography",
            Self::Image => "image",
            Self::Slice => "slice",
            Self::Icon => "icon",
            Self::Surface => "surface",
            Self::Border => "border",
            Self::Shadow => "shadow",
            Self::Gradient => "gradient",
            Self::Animation => "animation",
            Self::ControlState => "control_state",
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum UiVisualSupport {
    FrameworkSupported,
    DirectBevyAllowed,
    Unsupported,
}

impl UiVisualSupport {
    #[allow(dead_code)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::FrameworkSupported => "framework_supported",
            Self::DirectBevyAllowed => "direct_bevy_allowed",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Marks an intentional use of Bevy UI primitives that has no framework wrapper yet.
#[allow(dead_code)]
#[derive(Clone, Copy, Component, Debug, Eq, PartialEq)]
pub(crate) struct UiDirectBevyVisual {
    pub capability: UiVisualCapability,
    pub reason: &'static str,
}

impl UiDirectBevyVisual {
    #[allow(dead_code)]
    pub(crate) fn new(capability: UiVisualCapability, reason: &'static str) -> Self {
        assert!(
            !reason.trim().is_empty(),
            "direct Bevy visual usage requires a reason"
        );
        Self { capability, reason }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiVisualBudgetMetric {
    NodeCount,
    DecodedImageBytesEstimate,
    RenderPrimitiveEstimate,
    AdditionalEffectDrawCallUpperBound,
    MaterialCountEstimate,
    EffectOverdrawLayersUpperBound,
}

impl UiVisualBudgetMetric {
    const ORDERED: [Self; 6] = [
        Self::NodeCount,
        Self::DecodedImageBytesEstimate,
        Self::RenderPrimitiveEstimate,
        Self::AdditionalEffectDrawCallUpperBound,
        Self::MaterialCountEstimate,
        Self::EffectOverdrawLayersUpperBound,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NodeCount => "node_count",
            Self::DecodedImageBytesEstimate => "decoded_image_bytes_estimate",
            Self::RenderPrimitiveEstimate => "render_primitive_estimate",
            Self::AdditionalEffectDrawCallUpperBound => "additional_effect_draw_call_upper_bound",
            Self::MaterialCountEstimate => "material_count_estimate",
            Self::EffectOverdrawLayersUpperBound => "effect_overdraw_layers_upper_bound",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiVisualBudgetProfile {
    CompactHandheld,
    MediumTablet,
    ExpandedDesktop,
}

impl UiVisualBudgetProfile {
    pub(crate) const fn for_width_class(width_class: UiWidthClass) -> Self {
        match width_class {
            UiWidthClass::Compact => Self::CompactHandheld,
            UiWidthClass::Medium => Self::MediumTablet,
            UiWidthClass::Expanded => Self::ExpandedDesktop,
        }
    }

    pub(crate) const fn limits(self) -> UiVisualBudgetLimits {
        const MIB: u64 = 1024 * 1024;
        match self {
            Self::CompactHandheld => UiVisualBudgetLimits {
                node_count: 1_800,
                decoded_image_bytes_estimate: 64 * MIB,
                render_primitive_estimate: 1_800,
                additional_effect_draw_call_upper_bound: 32,
                material_count_estimate: 4,
                effect_overdraw_layers_upper_bound: 4,
            },
            Self::MediumTablet => UiVisualBudgetLimits {
                node_count: 2_000,
                decoded_image_bytes_estimate: 80 * MIB,
                render_primitive_estimate: 2_000,
                additional_effect_draw_call_upper_bound: 40,
                material_count_estimate: 6,
                effect_overdraw_layers_upper_bound: 5,
            },
            Self::ExpandedDesktop => UiVisualBudgetLimits {
                node_count: 2_200,
                decoded_image_bytes_estimate: 96 * MIB,
                render_primitive_estimate: 2_200,
                additional_effect_draw_call_upper_bound: 48,
                material_count_estimate: 8,
                effect_overdraw_layers_upper_bound: 6,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub(crate) struct UiVisualBudgetLimits {
    pub node_count: u64,
    pub decoded_image_bytes_estimate: u64,
    pub render_primitive_estimate: u64,
    pub additional_effect_draw_call_upper_bound: u64,
    pub material_count_estimate: u64,
    pub effect_overdraw_layers_upper_bound: u64,
}

impl UiVisualBudgetLimits {
    const fn value(self, metric: UiVisualBudgetMetric) -> u64 {
        match metric {
            UiVisualBudgetMetric::NodeCount => self.node_count,
            UiVisualBudgetMetric::DecodedImageBytesEstimate => self.decoded_image_bytes_estimate,
            UiVisualBudgetMetric::RenderPrimitiveEstimate => self.render_primitive_estimate,
            UiVisualBudgetMetric::AdditionalEffectDrawCallUpperBound => {
                self.additional_effect_draw_call_upper_bound
            }
            UiVisualBudgetMetric::MaterialCountEstimate => self.material_count_estimate,
            UiVisualBudgetMetric::EffectOverdrawLayersUpperBound => {
                self.effect_overdraw_layers_upper_bound
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub(crate) struct UiVisualBudgetUsage {
    pub node_count: u64,
    pub decoded_image_bytes_estimate: u64,
    pub unresolved_image_asset_count: u64,
    pub render_primitive_estimate: u64,
    pub additional_effect_draw_call_upper_bound: u64,
    pub material_count_estimate: u64,
    pub effect_overdraw_layers_upper_bound: u64,
}

impl UiVisualBudgetUsage {
    const fn value(self, metric: UiVisualBudgetMetric) -> u64 {
        match metric {
            UiVisualBudgetMetric::NodeCount => self.node_count,
            UiVisualBudgetMetric::DecodedImageBytesEstimate => self.decoded_image_bytes_estimate,
            UiVisualBudgetMetric::RenderPrimitiveEstimate => self.render_primitive_estimate,
            UiVisualBudgetMetric::AdditionalEffectDrawCallUpperBound => {
                self.additional_effect_draw_call_upper_bound
            }
            UiVisualBudgetMetric::MaterialCountEstimate => self.material_count_estimate,
            UiVisualBudgetMetric::EffectOverdrawLayersUpperBound => {
                self.effect_overdraw_layers_upper_bound
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiVisualBudgetStatus {
    Passed,
    Warning,
    Exceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiVisualBudgetFindingSeverity {
    Warning,
    Exceeded,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct UiVisualBudgetFinding {
    pub metric: UiVisualBudgetMetric,
    pub metric_id: &'static str,
    pub severity: UiVisualBudgetFindingSeverity,
    pub actual: u64,
    pub limit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct UiVisualBudgetReport {
    pub profile: UiVisualBudgetProfile,
    pub status: UiVisualBudgetStatus,
    pub accounting: &'static str,
    pub limits: UiVisualBudgetLimits,
    pub usage: UiVisualBudgetUsage,
    pub findings: Vec<UiVisualBudgetFinding>,
}

impl UiVisualBudgetReport {
    pub(crate) fn evaluate(profile: UiVisualBudgetProfile, usage: UiVisualBudgetUsage) -> Self {
        let limits = profile.limits();
        let mut findings = Vec::new();
        for metric in UiVisualBudgetMetric::ORDERED {
            let actual = usage.value(metric);
            let limit = limits.value(metric);
            let severity = if actual > limit {
                Some(UiVisualBudgetFindingSeverity::Exceeded)
            } else if limit > 0 && actual.saturating_mul(100) >= limit.saturating_mul(80) {
                Some(UiVisualBudgetFindingSeverity::Warning)
            } else {
                None
            };
            if let Some(severity) = severity {
                findings.push(UiVisualBudgetFinding {
                    metric,
                    metric_id: metric.as_str(),
                    severity,
                    actual,
                    limit,
                });
            }
        }
        let status = if findings
            .iter()
            .any(|finding| finding.severity == UiVisualBudgetFindingSeverity::Exceeded)
        {
            UiVisualBudgetStatus::Exceeded
        } else if findings.is_empty() {
            UiVisualBudgetStatus::Passed
        } else {
            UiVisualBudgetStatus::Warning
        };
        Self {
            profile,
            status,
            accounting: "development estimates; not measured GPU draw calls, VRAM, or pixel overdraw",
            limits,
            usage,
            findings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_capability_ids_are_stable() {
        let ids = [
            UiVisualCapability::Layout,
            UiVisualCapability::Typography,
            UiVisualCapability::Image,
            UiVisualCapability::Slice,
            UiVisualCapability::Icon,
            UiVisualCapability::Surface,
            UiVisualCapability::Border,
            UiVisualCapability::Shadow,
            UiVisualCapability::Gradient,
            UiVisualCapability::Animation,
            UiVisualCapability::ControlState,
        ]
        .map(UiVisualCapability::as_str);

        assert_eq!(
            ids,
            [
                "layout",
                "typography",
                "image",
                "slice",
                "icon",
                "surface",
                "border",
                "shadow",
                "gradient",
                "animation",
                "control_state",
            ]
        );
    }

    #[test]
    fn support_status_ids_are_stable() {
        assert_eq!(
            UiVisualSupport::FrameworkSupported.as_str(),
            "framework_supported"
        );
        assert_eq!(
            UiVisualSupport::DirectBevyAllowed.as_str(),
            "direct_bevy_allowed"
        );
        assert_eq!(UiVisualSupport::Unsupported.as_str(), "unsupported");
    }

    #[test]
    fn direct_bevy_marker_records_capability_and_reason() {
        let marker =
            UiDirectBevyVisual::new(UiVisualCapability::Shadow, "temporary BoxShadow experiment");

        assert_eq!(marker.capability, UiVisualCapability::Shadow);
        assert_eq!(marker.reason, "temporary BoxShadow experiment");
    }

    #[test]
    #[should_panic(expected = "direct Bevy visual usage requires a reason")]
    fn direct_bevy_marker_rejects_empty_reason() {
        UiDirectBevyVisual::new(UiVisualCapability::Gradient, "   ");
    }

    #[test]
    fn visual_budget_passes_below_warning_threshold() {
        let report = UiVisualBudgetReport::evaluate(
            UiVisualBudgetProfile::CompactHandheld,
            UiVisualBudgetUsage {
                node_count: 1_200,
                decoded_image_bytes_estimate: 32 * 1024 * 1024,
                render_primitive_estimate: 1_100,
                additional_effect_draw_call_upper_bound: 10,
                material_count_estimate: 2,
                effect_overdraw_layers_upper_bound: 2,
                ..default()
            },
        );

        assert_eq!(report.status, UiVisualBudgetStatus::Passed);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn visual_budget_warns_at_eighty_percent_and_exceeds_above_limit() {
        let limits = UiVisualBudgetProfile::CompactHandheld.limits();
        let warning = UiVisualBudgetReport::evaluate(
            UiVisualBudgetProfile::CompactHandheld,
            UiVisualBudgetUsage {
                node_count: limits.node_count * 4 / 5,
                ..default()
            },
        );
        let exceeded = UiVisualBudgetReport::evaluate(
            UiVisualBudgetProfile::CompactHandheld,
            UiVisualBudgetUsage {
                node_count: limits.node_count + 1,
                ..default()
            },
        );

        assert_eq!(warning.status, UiVisualBudgetStatus::Warning);
        assert_eq!(warning.findings[0].metric_id, "node_count");
        assert_eq!(exceeded.status, UiVisualBudgetStatus::Exceeded);
        assert_eq!(
            exceeded.findings[0].severity,
            UiVisualBudgetFindingSeverity::Exceeded
        );
    }

    #[test]
    fn visual_budget_findings_use_stable_metric_order() {
        let report = UiVisualBudgetReport::evaluate(
            UiVisualBudgetProfile::CompactHandheld,
            UiVisualBudgetUsage {
                node_count: u64::MAX,
                decoded_image_bytes_estimate: u64::MAX,
                render_primitive_estimate: u64::MAX,
                additional_effect_draw_call_upper_bound: u64::MAX,
                material_count_estimate: u64::MAX,
                effect_overdraw_layers_upper_bound: u64::MAX,
                ..default()
            },
        );

        assert_eq!(report.findings.len(), 6);
        assert_eq!(report.findings[0].metric_id, "node_count");
        assert_eq!(
            report.findings[5].metric_id,
            "effect_overdraw_layers_upper_bound"
        );
    }
}
