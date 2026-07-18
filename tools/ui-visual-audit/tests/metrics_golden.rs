mod common;

use common::TestRepository;
use image::GenericImageView;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use ui_visual_audit::{
    ComparisonErrorCode, ComparisonExitCode, DiffAnalysisRequest, DiffAnalysisStatus,
    analyze_aligned_diff,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenFile {
    schema_version: u32,
    algorithm_version: String,
    cases: Vec<GoldenCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenCase {
    id: String,
    width: u32,
    height: u32,
    fixture: String,
    #[serde(default)]
    expected_metrics_sha256: String,
}

fn golden_file() -> GoldenFile {
    serde_json::from_slice(include_bytes!(
        "../fixtures/comparison/ui-diff-metrics-v1.golden-cases.json"
    ))
    .unwrap()
}

fn fixture(case: &GoldenCase) -> (Vec<u8>, Vec<u8>) {
    let pixels = case.width as usize * case.height as usize;
    let solid = |rgba: [u8; 4]| rgba.repeat(pixels);
    match case.fixture.as_str() {
        "solid_color_bias" => (solid([100, 110, 120, 255]), solid([104, 114, 124, 255])),
        "one_pixel_shift" => {
            let mut reference = solid([240, 240, 240, 255]);
            let mut actual = reference.clone();
            fill_rect(&mut reference, case.width, 8, 4, 8, 8, [20, 20, 20, 255]);
            fill_rect(&mut actual, case.width, 9, 4, 8, 8, [20, 20, 20, 255]);
            (reference, actual)
        }
        "font_antialias_edge" => {
            let mut reference = solid([255, 255, 255, 255]);
            let mut actual = reference.clone();
            for y in 2..14 {
                set_pixel(&mut reference, case.width, 6, y, [200, 200, 200, 255]);
                set_pixel(&mut reference, case.width, 7, y, [80, 80, 80, 255]);
                set_pixel(&mut reference, case.width, 8, y, [0, 0, 0, 255]);
                set_pixel(&mut actual, case.width, 6, y, [204, 204, 204, 255]);
                set_pixel(&mut actual, case.width, 7, y, [88, 88, 88, 255]);
                set_pixel(&mut actual, case.width, 8, y, [0, 0, 0, 255]);
            }
            (reference, actual)
        }
        "missing_control" => {
            let mut reference = solid([235, 235, 235, 255]);
            fill_rect(&mut reference, case.width, 8, 8, 16, 8, [35, 70, 120, 255]);
            (reference, solid([235, 235, 235, 255]))
        }
        "large_background_change" => (solid([240, 240, 240, 255]), solid([200, 205, 210, 255])),
        "alpha_change" => (solid([80, 120, 160, 255]), solid([80, 120, 160, 235])),
        unknown => panic!("unknown fixture {unknown}"),
    }
}

fn fill_rect(
    rgba: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    rect_width: u32,
    rect_height: u32,
    color: [u8; 4],
) {
    for target_y in y..y + rect_height {
        for target_x in x..x + rect_width {
            set_pixel(rgba, width, target_x, target_y, color);
        }
    }
}

fn set_pixel(rgba: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let offset = ((y * width + x) * 4) as usize;
    rgba[offset..offset + 4].copy_from_slice(&color);
}

fn config(repository: &TestRepository) -> PathBuf {
    repository.write_bytes(
        "metrics.config.json",
        include_bytes!("../fixtures/comparison/ui-diff-metrics-v1.config.json"),
    )
}

fn request(
    repository: &TestRepository,
    reference: PathBuf,
    actual: PathBuf,
    config: PathBuf,
    output: &str,
) -> DiffAnalysisRequest {
    DiffAnalysisRequest {
        repository_root: repository.root.clone(),
        allowed_input_roots: vec![repository.inputs.clone()],
        allowed_output_root: repository.outputs.clone(),
        reference,
        actual,
        config,
        output_directory: repository.outputs.join(output),
    }
}

#[test]
fn textual_fixtures_pin_every_serialized_metric() {
    let golden = golden_file();
    assert_eq!(golden.schema_version, 1);
    assert_eq!(golden.algorithm_version, "ui_diff_metrics_v1");
    for case in &golden.cases {
        let repository = TestRepository::new();
        let (reference, actual) = fixture(case);
        let reference = repository.write_png("reference.png", case.width, case.height, &reference);
        let actual = repository.write_png("actual.png", case.width, case.height, &actual);
        let outcome = analyze_aligned_diff(&request(
            &repository,
            reference,
            actual,
            config(&repository),
            "golden",
        ))
        .unwrap();
        assert_eq!(outcome.status(), DiffAnalysisStatus::Analyzed);
        let metrics = outcome.report.metrics.unwrap();
        match case.id.as_str() {
            "solid_color_bias" => {
                assert_eq!(metrics.raw.changed_pixels, 256);
                assert_eq!(metrics.raw.maximum_absolute_channel_error, 4);
                assert_eq!(
                    metrics.raw.channels.red.mean_absolute_error_millionths,
                    4_000_000
                );
                assert_eq!(metrics.tolerated.changed_pixels, 256);
            }
            "one_pixel_shift" => {
                assert_eq!(metrics.raw.changed_pixels, 16);
                assert_eq!(metrics.tolerated.changed_pixels, 16);
                assert!(metrics.categories.geometry_edges.mismatched_edge_pixels > 0);
            }
            "font_antialias_edge" => {
                assert_eq!(metrics.raw.changed_pixels, 24);
                assert_eq!(metrics.tolerated.changed_pixels, 0);
                assert_eq!(metrics.tolerated.ignored_matching_edge_antialias_pixels, 24);
            }
            "missing_control" => {
                assert_eq!(metrics.raw.changed_pixels, 128);
                assert_eq!(
                    metrics
                        .categories
                        .large_area_content
                        .largest_component_pixels,
                    128
                );
            }
            "large_background_change" => {
                assert_eq!(metrics.raw.changed_pixels, 768);
                assert_eq!(metrics.categories.large_area_content.covered_pixels, 768);
            }
            "alpha_change" => {
                assert_eq!(metrics.alpha.changed_pixels, 64);
                assert_eq!(metrics.alpha.mean_absolute_error_millionths, 20_000_000);
            }
            _ => unreachable!(),
        }
        let bytes = serde_json::to_vec(&metrics).unwrap();
        let hash = format!("{:x}", Sha256::digest(bytes));
        println!("{} {}", case.id, hash);
        if !case.expected_metrics_sha256.is_empty() {
            assert_eq!(hash, case.expected_metrics_sha256, "case {}", case.id);
        }
    }
}

#[test]
fn artifacts_have_fixed_dimensions_and_repeatable_pixels() {
    let case = &golden_file().cases[1];
    let repository = TestRepository::new();
    let (reference_rgba, actual_rgba) = fixture(case);
    let reference = repository.write_png("reference.png", case.width, case.height, &reference_rgba);
    let actual = repository.write_png("actual.png", case.width, case.height, &actual_rgba);
    let first = analyze_aligned_diff(&request(
        &repository,
        reference.clone(),
        actual.clone(),
        config(&repository),
        "first",
    ))
    .unwrap();
    let second = analyze_aligned_diff(&request(
        &repository,
        reference,
        actual,
        config(&repository),
        "second",
    ))
    .unwrap();
    assert_eq!(first.report.metrics, second.report.metrics);
    assert_eq!(first.report.schema_version, 2);
    assert_eq!(first.report.artifacts.len(), 5);
    for filename in ["overlay.png", "heatmap.png", "binary-diff.png"] {
        let artifact_type = filename.trim_end_matches(".png").replace('-', "_");
        let artifact = first
            .report
            .artifacts
            .iter()
            .find(|artifact| artifact.artifact_type == artifact_type)
            .unwrap();
        let artifact_bytes = fs::read(repository.outputs.join("first").join(filename)).unwrap();
        assert_eq!(artifact.byte_length, Some(artifact_bytes.len() as u64));
        assert_eq!(
            artifact.sha256.as_deref(),
            Some(format!("{:x}", Sha256::digest(&artifact_bytes)).as_str())
        );
        let first_image = image::open(repository.outputs.join("first").join(filename)).unwrap();
        assert_eq!(first_image.dimensions(), (case.width, case.height));
        assert_eq!(
            fs::read(repository.outputs.join("first").join(filename)).unwrap(),
            fs::read(repository.outputs.join("second").join(filename)).unwrap()
        );
    }
    assert_eq!(
        image::open(repository.outputs.join("first/side-by-side.png"))
            .unwrap()
            .dimensions(),
        (case.width * 2, case.height)
    );
    let binary = image::open(repository.outputs.join("first/binary-diff.png"))
        .unwrap()
        .into_rgba8();
    assert_eq!(
        binary.pixels().filter(|pixel| pixel[0] == 255).count(),
        16,
        "a 1px layout shift must remain visible after tolerance"
    );
}

#[test]
fn dimension_and_pixel_format_fail_with_distinct_stable_codes() {
    let repository = TestRepository::new();
    let reference = repository.write_png("reference.png", 8, 8, &[0, 0, 0, 255].repeat(64));
    let actual = repository.write_png("actual.png", 7, 8, &[0, 0, 0, 255].repeat(56));
    let outcome = analyze_aligned_diff(&request(
        &repository,
        reference.clone(),
        actual,
        config(&repository),
        "dimensions",
    ))
    .unwrap();
    assert_eq!(outcome.exit_code, ComparisonExitCode::ComparisonFailure);
    assert_eq!(outcome.report.status, DiffAnalysisStatus::ComparisonFailed);
    assert_eq!(
        outcome.report.failure.unwrap().code,
        ComparisonErrorCode::DimensionsMismatch
    );
    assert!(
        repository
            .outputs
            .join("dimensions/diff-metrics-report.json")
            .is_file()
    );
    assert!(!repository.outputs.join("dimensions/heatmap.png").exists());

    let rgb = repository.write_rgb_png("rgb.png", 8, 8, &[0, 0, 0].repeat(64));
    let error = analyze_aligned_diff(&request(
        &repository,
        reference,
        rgb,
        config(&repository),
        "rgb",
    ))
    .unwrap_err();
    assert_eq!(error.exit_code(), ComparisonExitCode::InputFailure);
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::ImageUnsupportedFormat
    );
}

#[test]
fn aligned_alpha_contract_rejects_hidden_rgb_and_accepts_canonical_transparency() {
    let repository = TestRepository::new();
    let mut hidden = [10, 20, 30, 255].repeat(64);
    hidden[0..4].copy_from_slice(&[9, 0, 0, 0]);
    let hidden = repository.write_png("hidden.png", 8, 8, &hidden);
    let opaque = repository.write_png("opaque.png", 8, 8, &[10, 20, 30, 255].repeat(64));
    let error = analyze_aligned_diff(&request(
        &repository,
        hidden,
        opaque.clone(),
        config(&repository),
        "hidden",
    ))
    .unwrap_err();
    assert_eq!(error.exit_code(), ComparisonExitCode::InputFailure);
    assert_eq!(error.failure.code, ComparisonErrorCode::AlignedAlphaInvalid);
    assert_eq!(
        serde_json::to_value(error.failure.code).unwrap(),
        "aligned_alpha_invalid"
    );

    let mut canonical = [10, 20, 30, 255].repeat(64);
    canonical[0..4].copy_from_slice(&[0, 0, 0, 0]);
    let reference = repository.write_png("canonical-reference.png", 8, 8, &canonical);
    let actual = repository.write_png("canonical-actual.png", 8, 8, &canonical);
    let outcome = analyze_aligned_diff(&request(
        &repository,
        reference,
        actual,
        config(&repository),
        "canonical",
    ))
    .unwrap();
    assert_eq!(outcome.exit_code, ComparisonExitCode::Success);
    assert_eq!(outcome.report.status, DiffAnalysisStatus::Analyzed);
    assert_eq!(outcome.report.metrics.unwrap().raw.changed_pixels, 0);
}

trait OutcomeStatus {
    fn status(&self) -> DiffAnalysisStatus;
}

impl OutcomeStatus for ui_visual_audit::DiffAnalysisOutcome {
    fn status(&self) -> DiffAnalysisStatus {
        self.report.status
    }
}
