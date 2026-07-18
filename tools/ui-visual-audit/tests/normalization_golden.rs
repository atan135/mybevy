mod common;

use common::TestRepository;
use image::{ExtendedColorType, ImageEncoder, codecs::jpeg::JpegEncoder, codecs::png::PngEncoder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use ui_visual_audit::{
    ComparisonErrorCode, ComparisonExitCode, NormalizationRequest, NormalizationStatus,
    normalize_and_align,
};

fn patterned_rgba(width: u32, height: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&[
                (x * 29 + y * 7) as u8,
                (x * 3 + y * 41) as u8,
                (x * 17 + y * 11) as u8,
                if (x + y) % 5 == 0 { 180 } else { 255 },
            ]);
        }
    }
    rgba
}

fn shifted_right(source: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut shifted = vec![9_u8; source.len()];
    for pixel in shifted.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    for y in 0..height {
        for x in 1..width {
            let target = ((y * width + x) * 4) as usize;
            let source_index = ((y * width + x - 1) * 4) as usize;
            shifted[target..target + 4].copy_from_slice(&source[source_index..source_index + 4]);
        }
    }
    shifted
}

fn base_manifest() -> Value {
    json!({
        "schema_version": 1,
        "algorithm_version": "normalize_align_v1",
        "orientation_policy": "apply_exif",
        "color_policy": "srgb_only",
        "alpha_policy": "straight_zero_transparent_rgb",
        "reference": { "crop": { "kind": "none" } },
        "actual": { "crop": { "kind": "none" } },
        "alignment": {
            "mode": "integer_search",
            "maximum_translation": { "x": 2, "y": 2 }
        }
    })
}

fn write_manifest(repository: &TestRepository, name: &str, manifest: &Value) -> PathBuf {
    repository.write_bytes(name, &serde_json::to_vec_pretty(manifest).unwrap())
}

fn request(
    repository: &TestRepository,
    reference: PathBuf,
    actual: PathBuf,
    manifest: PathBuf,
    output_name: &str,
) -> NormalizationRequest {
    NormalizationRequest {
        repository_root: repository.root.clone(),
        allowed_input_roots: vec![repository.inputs.clone()],
        allowed_output_root: repository.outputs.clone(),
        reference,
        actual,
        normalization_manifest: manifest,
        output_directory: repository.outputs.join(output_name),
    }
}

#[test]
fn integer_alignment_persists_intermediates_and_traceable_coordinates() {
    let repository = TestRepository::new();
    let source = patterned_rgba(8, 6);
    let reference = repository.write_png("reference.png", 8, 6, &source);
    let actual = repository.write_png("actual.png", 8, 6, &shifted_right(&source, 8, 6));
    let manifest = write_manifest(&repository, "normalization.json", &base_manifest());
    let outcome = normalize_and_align(&request(
        &repository,
        reference,
        actual,
        manifest,
        "aligned",
    ))
    .unwrap();

    assert_eq!(outcome.exit_code, ComparisonExitCode::Success);
    assert_eq!(outcome.report.status, NormalizationStatus::Passed);
    let alignment = outcome.report.alignment.unwrap();
    assert_eq!(alignment.selected_translation.x, -1);
    assert_eq!(alignment.selected_translation.y, 0);
    assert_eq!(alignment.scale_x_millionths, 1_000_000);
    assert_eq!(alignment.aligned_dimensions.width, 7);
    assert_eq!(alignment.mean_absolute_channel_error_millionths, Some(0));
    assert_eq!(outcome.report.artifacts.len(), 7);
    for artifact in &outcome.report.artifacts {
        assert!(PathBuf::from(&artifact.path).is_file());
    }
    let reference_mapping = outcome.report.reference.coordinate_mapping.unwrap();
    let actual_mapping = outcome.report.actual.coordinate_mapping.unwrap();
    let reference_bounds = ui_visual_audit::PixelRect {
        x: 2,
        y: 2,
        width: 2,
        height: 2,
    };
    let actual_bounds = ui_visual_audit::PixelRect {
        x: 3,
        ..reference_bounds
    };
    let aligned = reference_mapping.map_original_rect_to_aligned(reference_bounds);
    assert_eq!(
        aligned,
        actual_mapping.map_original_rect_to_aligned(actual_bounds)
    );
    assert_eq!(
        reference_mapping.map_aligned_rect_to_original(aligned),
        reference_bounds
    );
    assert_eq!(
        actual_mapping.map_aligned_rect_to_original(aligned),
        actual_bounds
    );
}

#[test]
fn explicit_crop_kinds_are_recorded_without_resize() {
    let repository = TestRepository::new();
    let source = patterned_rgba(8, 6);
    let reference = repository.write_png("reference.png", 8, 6, &source);
    let actual = repository.write_png("actual.png", 8, 6, &source);
    let mut manifest = base_manifest();
    manifest["reference"]["crop"] =
        json!({"kind":"system_ui","left":1,"top":1,"right":0,"bottom":0});
    manifest["actual"]["crop"] = json!({"kind":"safe_area","left":1,"top":1,"right":0,"bottom":0});
    manifest["alignment"] = json!({
        "mode":"none",
        "maximum_translation":{"x":0,"y":0}
    });
    let manifest = write_manifest(&repository, "crops.json", &manifest);
    let outcome = normalize_and_align(&request(
        &repository,
        reference.clone(),
        actual.clone(),
        manifest,
        "crops",
    ))
    .unwrap();
    assert_eq!(
        outcome.report.reference.crop.kind,
        ui_visual_audit::CropKind::SystemUi
    );
    assert_eq!(
        outcome.report.actual.crop.kind,
        ui_visual_audit::CropKind::SafeArea
    );
    assert_eq!(outcome.report.reference.crop.after_dimensions.width, 7);
    assert_eq!(
        outcome.report.alignment.unwrap().scale_x_millionths,
        1_000_000
    );

    let mut fixed_manifest = base_manifest();
    fixed_manifest["reference"]["crop"] =
        json!({"kind":"fixed_border","left":1,"top":0,"right":1,"bottom":0});
    fixed_manifest["actual"]["crop"] =
        json!({"kind":"fixed_border","left":1,"top":0,"right":1,"bottom":0});
    fixed_manifest["alignment"] = json!({
        "mode":"none",
        "maximum_translation":{"x":0,"y":0}
    });
    let fixed_manifest = write_manifest(&repository, "fixed-border.json", &fixed_manifest);
    let fixed_outcome = normalize_and_align(&request(
        &repository,
        reference,
        actual,
        fixed_manifest,
        "fixed-border",
    ))
    .unwrap();
    assert_eq!(
        fixed_outcome.report.reference.crop.kind,
        ui_visual_audit::CropKind::FixedBorder
    );
    assert_eq!(
        fixed_outcome.report.reference.crop.after_dimensions.width,
        6
    );
}

#[test]
fn dimensions_aspect_ratio_and_maximum_translation_fail_separately() {
    let repository = TestRepository::new();
    let reference = repository.write_png("reference.png", 8, 6, &patterned_rgba(8, 6));
    let actual = repository.write_png("actual.png", 7, 6, &patterned_rgba(7, 6));
    let manifest = write_manifest(&repository, "dimensions.json", &base_manifest());
    let outcome = normalize_and_align(&request(
        &repository,
        reference.clone(),
        actual,
        manifest,
        "dimension-failure",
    ))
    .unwrap();
    assert_eq!(outcome.exit_code, ComparisonExitCode::ComparisonFailure);
    assert_eq!(
        outcome.report.failure.unwrap().code,
        ComparisonErrorCode::AspectRatioMismatch
    );
    assert!(
        !repository
            .outputs
            .join("dimension-failure/aligned-reference.png")
            .exists()
    );

    let scaled = repository.write_png("same-ratio.png", 4, 3, &patterned_rgba(4, 3));
    let manifest = write_manifest(&repository, "physical-size.json", &base_manifest());
    let outcome = normalize_and_align(&request(
        &repository,
        reference.clone(),
        scaled,
        manifest,
        "physical-size-failure",
    ))
    .unwrap();
    assert_eq!(
        outcome.report.failure.unwrap().code,
        ComparisonErrorCode::DimensionsMismatch
    );

    let actual = repository.write_png("actual-wide.png", 8, 6, &patterned_rgba(8, 6));
    let mut manifest = base_manifest();
    manifest["alignment"] = json!({
        "mode":"declared_integer",
        "maximum_translation":{"x":2,"y":1},
        "declared_translation":{"x":3,"y":0}
    });
    let manifest = write_manifest(&repository, "maximum.json", &manifest);
    let outcome = normalize_and_align(&request(
        &repository,
        reference,
        actual,
        manifest,
        "maximum-failure",
    ))
    .unwrap();
    assert_eq!(outcome.exit_code, ComparisonExitCode::ComparisonFailure);
    assert_eq!(
        outcome.report.failure.unwrap().code,
        ComparisonErrorCode::MaximumTranslationExceeded
    );
}

#[test]
fn alpha_and_supported_rgb_conversion_produce_canonical_rgba8() {
    let repository = TestRepository::new();
    let mut rgba = patterned_rgba(4, 4);
    rgba[0..4].copy_from_slice(&[91, 92, 93, 0]);
    let reference = repository.write_png("alpha.png", 4, 4, &rgba);
    let rgb: Vec<u8> = patterned_rgba(4, 4)
        .chunks_exact(4)
        .flat_map(|pixel| pixel[..3].iter().copied())
        .collect();
    let actual = repository.write_rgb_png("rgb.png", 4, 4, &rgb);
    let mut manifest = base_manifest();
    manifest["alignment"] = json!({
        "mode":"none",
        "maximum_translation":{"x":0,"y":0}
    });
    let manifest = write_manifest(&repository, "alpha-rgb.json", &manifest);
    let outcome = normalize_and_align(&request(
        &repository,
        reference,
        actual,
        manifest,
        "alpha-rgb",
    ))
    .unwrap();
    assert_eq!(outcome.report.status, NormalizationStatus::Passed);
    assert_eq!(outcome.report.reference.pixel_format, "rgba8");
    assert_eq!(outcome.report.actual.source_alpha, "opaque");
    assert_eq!(outcome.report.actual.output_color_space, "srgb");
    let normalized = image::open(
        repository
            .outputs
            .join("alpha-rgb/normalized-reference.png"),
    )
    .unwrap()
    .into_rgba8();
    assert_eq!(normalized.get_pixel(0, 0).0, [0, 0, 0, 0]);
}

#[test]
fn all_exif_orientations_normalize_pixels_dimensions_and_coordinate_maps() {
    let repository = TestRepository::new();
    let mut manifest = base_manifest();
    manifest["alignment"] = json!({
        "mode":"none",
        "maximum_translation":{"x":0,"y":0}
    });
    let manifest = write_manifest(&repository, "exif.json", &manifest);
    for orientation in 1..=8 {
        let jpeg = jpeg_with_orientation(3, 2, &patterned_rgb(3, 2), orientation);
        let reference = repository.write_bytes(&format!("reference-{orientation}.jpg"), &jpeg);
        let actual = repository.write_bytes(&format!("actual-{orientation}.jpg"), &jpeg);
        let output_name = format!("exif-{orientation}");
        let outcome = normalize_and_align(&request(
            &repository,
            reference,
            actual,
            manifest.clone(),
            &output_name,
        ))
        .unwrap();
        let report = &outcome.report.reference;
        assert_eq!(report.exif_orientation, orientation);
        assert_eq!(report.original_dimensions.width, 3);
        assert_eq!(report.original_dimensions.height, 2);
        let expected_size = if orientation >= 5 { (2, 3) } else { (3, 2) };
        assert_eq!(report.oriented_dimensions.width, expected_size.0);
        assert_eq!(report.oriented_dimensions.height, expected_size.1);

        let decoded = image::load_from_memory(&jpeg).unwrap().into_rgba8();
        let normalized = image::open(
            repository
                .outputs
                .join(&output_name)
                .join("normalized-reference.png"),
        )
        .unwrap()
        .into_rgba8();
        assert_eq!(normalized.dimensions(), expected_size);
        let mapping = report.coordinate_mapping.as_ref().unwrap();
        for y in 0..decoded.height() {
            for x in 0..decoded.width() {
                let source_pixel = ui_visual_audit::PixelRect {
                    x: i64::from(x),
                    y: i64::from(y),
                    width: 1,
                    height: 1,
                };
                let target_pixel = mapping.map_original_rect_to_aligned(source_pixel);
                assert_eq!((target_pixel.width, target_pixel.height), (1, 1));
                assert_eq!(
                    normalized
                        .get_pixel(target_pixel.x as u32, target_pixel.y as u32)
                        .0,
                    decoded.get_pixel(x, y).0,
                    "orientation {orientation} pixel ({x}, {y})"
                );
                assert_eq!(
                    mapping.map_aligned_rect_to_original(target_pixel),
                    source_pixel
                );
            }
        }
    }
}

#[test]
fn transparent_and_near_blank_inputs_have_stable_codes() {
    let repository = TestRepository::new();
    let transparent = repository.write_png("transparent.png", 4, 4, &[0; 64]);
    let pattern = repository.write_png("pattern.png", 4, 4, &patterned_rgba(4, 4));
    let manifest = write_manifest(&repository, "transparent.json", &base_manifest());
    let outcome = normalize_and_align(&request(
        &repository,
        transparent,
        pattern.clone(),
        manifest,
        "transparent",
    ))
    .unwrap();
    assert_eq!(
        outcome.report.failure.unwrap().code,
        ComparisonErrorCode::ImageAllTransparent
    );

    let blank = repository.write_png("blank.png", 8, 8, &[12, 12, 12, 255].repeat(64));
    let manifest = write_manifest(&repository, "blank-manifest.json", &base_manifest());
    let outcome =
        normalize_and_align(&request(&repository, blank, pattern, manifest, "blank")).unwrap();
    assert_eq!(
        outcome.report.failure.unwrap().code,
        ComparisonErrorCode::ImageNearBlank
    );
}

#[test]
fn hash_identity_distinguishes_equal_roles_swaps_and_single_role_mismatches() {
    let repository = TestRepository::new();
    let reference_bytes = encode_png(4, 4, &patterned_rgba(4, 4));
    let actual_bytes = encode_png(4, 4, &shifted_right(&patterned_rgba(4, 4), 4, 4));
    let reference = repository.write_bytes("identity-reference.png", &reference_bytes);
    let actual = repository.write_bytes("identity-actual.png", &actual_bytes);

    let mut same_manifest = base_manifest();
    same_manifest["reference"]["expected_sha256"] = json!(sha256(&reference_bytes));
    same_manifest["actual"]["expected_sha256"] = json!(sha256(&reference_bytes));
    let same_manifest = write_manifest(&repository, "same-hash.json", &same_manifest);
    let same_outcome = normalize_and_align(&request(
        &repository,
        reference.clone(),
        reference.clone(),
        same_manifest,
        "same-hash",
    ))
    .unwrap();
    assert_eq!(same_outcome.report.status, NormalizationStatus::Passed);
    assert!(same_outcome.report.failure.is_none());

    let mut swapped_manifest = base_manifest();
    swapped_manifest["reference"]["expected_sha256"] = json!(sha256(&actual_bytes));
    swapped_manifest["actual"]["expected_sha256"] = json!(sha256(&reference_bytes));
    let swapped_manifest = write_manifest(&repository, "swapped.json", &swapped_manifest);
    let swapped_outcome = normalize_and_align(&request(
        &repository,
        reference.clone(),
        actual.clone(),
        swapped_manifest,
        "swapped",
    ))
    .unwrap();
    assert_eq!(
        swapped_outcome.report.failure.unwrap().code,
        ComparisonErrorCode::InputsSwapped
    );

    let mut mismatch_manifest = base_manifest();
    mismatch_manifest["reference"]["expected_sha256"] = json!(sha256(&reference_bytes));
    mismatch_manifest["actual"]["expected_sha256"] = json!("f".repeat(64));
    let mismatch_manifest = write_manifest(&repository, "single-mismatch.json", &mismatch_manifest);
    let mismatch_outcome = normalize_and_align(&request(
        &repository,
        reference,
        actual,
        mismatch_manifest,
        "single-mismatch",
    ))
    .unwrap();
    assert_eq!(
        mismatch_outcome.report.failure.unwrap().code,
        ComparisonErrorCode::InputIdentityMismatch
    );
}

#[test]
fn unknown_icc_profile_is_rejected_instead_of_claiming_conversion() {
    let repository = TestRepository::new();
    let mut png = encode_png(4, 4, &patterned_rgba(4, 4));
    let iend = png.windows(4).position(|window| window == b"IEND").unwrap() - 4;
    let mut chunk = Vec::new();
    chunk.extend_from_slice(&4_u32.to_be_bytes());
    chunk.extend_from_slice(b"iCCP");
    chunk.extend_from_slice(b"fake");
    chunk.extend_from_slice(&0_u32.to_be_bytes());
    png.splice(iend..iend, chunk);
    let reference = repository.write_bytes("profiled.png", &png);
    let actual = repository.write_png("actual.png", 4, 4, &patterned_rgba(4, 4));
    let manifest = write_manifest(&repository, "icc.json", &base_manifest());
    let error =
        normalize_and_align(&request(&repository, reference, actual, manifest, "icc")).unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::UnsupportedColorProfile
    );
    assert_eq!(error.exit_code(), ComparisonExitCode::InputFailure);
}

fn patterned_rgb(width: u32, height: u32) -> Vec<u8> {
    patterned_rgba(width, height)
        .chunks_exact(4)
        .flat_map(|pixel| pixel[..3].iter().copied())
        .collect()
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(rgba, width, height, ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

fn jpeg_with_orientation(width: u32, height: u32, rgb: &[u8], orientation: u16) -> Vec<u8> {
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, 100)
        .write_image(rgb, width, height, ExtendedColorType::Rgb8)
        .unwrap();
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II");
    tiff.extend_from_slice(&42_u16.to_le_bytes());
    tiff.extend_from_slice(&8_u32.to_le_bytes());
    tiff.extend_from_slice(&1_u16.to_le_bytes());
    tiff.extend_from_slice(&0x0112_u16.to_le_bytes());
    tiff.extend_from_slice(&3_u16.to_le_bytes());
    tiff.extend_from_slice(&1_u32.to_le_bytes());
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&0_u16.to_le_bytes());
    tiff.extend_from_slice(&0_u32.to_le_bytes());
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&tiff);
    let mut result = encoded[..2].to_vec();
    result.extend_from_slice(&[0xff, 0xe1]);
    result.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    result.extend_from_slice(&payload);
    result.extend_from_slice(&encoded[2..]);
    result
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
