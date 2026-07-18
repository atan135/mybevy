mod common;

use common::{TestRepository, decode_hex};
use serde::Deserialize;
use ui_visual_audit::{ComparisonErrorCode, ComparisonExitCode, ComparisonStatus, compare_images};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenFixtures {
    schema_version: u32,
    cases: Vec<GoldenCase>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum GoldenCase {
    RgbaPair {
        id: String,
        reference: Raster,
        actual: Raster,
        expected_status: ComparisonStatus,
    },
    InvalidInput {
        id: String,
        extension: String,
        bytes_hex: String,
        expected_code: ComparisonErrorCode,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raster {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

#[test]
fn versioned_golden_cases_cover_minimal_comparison_and_input_boundaries() {
    let fixtures: GoldenFixtures =
        serde_json::from_str(include_str!("../fixtures/comparison/golden-cases.json")).unwrap();
    assert_eq!(fixtures.schema_version, 1);
    assert_eq!(fixtures.cases.len(), 7);

    for case in fixtures.cases {
        let repository = TestRepository::new();
        let config = repository.write_config(0.0);
        match case {
            GoldenCase::RgbaPair {
                id,
                reference,
                actual,
                expected_status,
            } => {
                let reference_path = repository.write_png(
                    "reference.png",
                    reference.width,
                    reference.height,
                    &reference.rgba,
                );
                let actual_path =
                    repository.write_png("actual.png", actual.width, actual.height, &actual.rgba);
                let outcome =
                    compare_images(&repository.request(reference_path, actual_path, config, &id))
                        .unwrap();
                assert_eq!(outcome.report.status, expected_status, "case {id}");
                assert_eq!(
                    outcome.exit_code,
                    match expected_status {
                        ComparisonStatus::Passed => ComparisonExitCode::Success,
                        ComparisonStatus::ComparisonFailed => {
                            ComparisonExitCode::ComparisonFailure
                        }
                        ComparisonStatus::ThresholdFailed => {
                            ComparisonExitCode::ThresholdFailure
                        }
                    },
                    "case {id}"
                );
            }
            GoldenCase::InvalidInput {
                id,
                extension,
                bytes_hex,
                expected_code,
            } => {
                let reference = repository
                    .write_bytes(&format!("reference.{extension}"), &decode_hex(&bytes_hex));
                let actual = repository.write_png("actual.png", 1, 1, &[0, 0, 0, 255]);
                let error = compare_images(&repository.request(reference, actual, config, &id))
                    .unwrap_err();
                assert_eq!(error.failure.code, expected_code, "case {id}");
                assert_eq!(error.exit_code(), ComparisonExitCode::InputFailure);
            }
        }
    }
}

#[test]
fn mask_is_optional_but_when_present_scopes_the_exact_comparison() {
    let repository = TestRepository::new();
    let reference =
        repository.write_png("reference.png", 2, 1, &[10, 20, 30, 255, 10, 20, 30, 255]);
    let actual = repository.write_png("actual.png", 2, 1, &[10, 20, 30, 255, 255, 255, 255, 255]);
    let mask = repository.write_png("mask.png", 2, 1, &[255, 255, 255, 255, 0, 0, 0, 255]);
    let config = repository.write_config(0.0);
    let mut request = repository.request(reference, actual, config, "masked");
    request.mask = Some(mask);

    let outcome = compare_images(&request).unwrap();
    assert_eq!(outcome.report.status, ComparisonStatus::Passed);
    let metrics = outcome.report.metrics.unwrap();
    assert_eq!(metrics.evaluated_pixels, 1);
    assert_eq!(metrics.changed_pixels, 0);
}

#[test]
fn committed_exact_v1_configuration_fixture_matches_the_strict_schema() {
    let config: ui_visual_audit::ComparisonConfig =
        serde_json::from_str(include_str!("../fixtures/comparison/exact-v1.config.json")).unwrap();
    assert_eq!(config.schema_version, 1);
    assert_eq!(config.algorithm_version, "exact_rgba_v1");
    assert_eq!(config.max_changed_pixel_ratio, 0.0);
}

#[test]
fn canonical_input_and_output_roots_reject_escape_attempts() {
    let repository = TestRepository::new();
    let reference = repository.write_png("reference.png", 1, 1, &[0, 0, 0, 255]);
    let actual = repository.write_png("actual.png", 1, 1, &[0, 0, 0, 255]);
    let config = repository.write_config(0.0);
    let external = tempfile::tempdir().unwrap();
    let external_image = external.path().join("external.png");
    std::fs::copy(&reference, &external_image).unwrap();

    let input_error = compare_images(&repository.request(
        external_image,
        actual.clone(),
        config.clone(),
        "outside-input",
    ))
    .unwrap_err();
    assert_eq!(
        input_error.failure.code,
        ComparisonErrorCode::InputOutsideAllowedRoot
    );

    let mut output_request = repository.request(reference, actual, config, "unused");
    output_request.output_directory = external.path().join("outside-output");
    let output_error = compare_images(&output_request).unwrap_err();
    assert_eq!(
        output_error.failure.code,
        ComparisonErrorCode::OutputOutsideAllowedRoot
    );
}

#[test]
fn output_directory_errors_distinguish_file_and_nonempty_directory() {
    let repository = TestRepository::new();
    let reference = repository.write_png("reference.png", 1, 1, &[0, 0, 0, 255]);
    let actual = repository.write_png("actual.png", 1, 1, &[0, 0, 0, 255]);
    let config = repository.write_config(0.0);

    let output_file = repository.outputs.join("not-a-directory");
    std::fs::write(&output_file, b"file").unwrap();
    let mut file_request = repository.request(
        reference.clone(),
        actual.clone(),
        config.clone(),
        "unused-file",
    );
    file_request.output_directory = output_file;
    assert_eq!(
        compare_images(&file_request).unwrap_err().failure.code,
        ComparisonErrorCode::OutputNotDirectory
    );

    let nonempty = repository.outputs.join("nonempty");
    std::fs::create_dir(&nonempty).unwrap();
    std::fs::write(nonempty.join("unrelated.txt"), b"occupied").unwrap();
    let mut nonempty_request = repository.request(reference, actual, config, "unused-nonempty");
    nonempty_request.output_directory = nonempty;
    assert_eq!(
        compare_images(&nonempty_request).unwrap_err().failure.code,
        ComparisonErrorCode::OutputDirectoryNotEmpty
    );
}
