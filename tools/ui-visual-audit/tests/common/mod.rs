use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use std::{fs, path::PathBuf};
use tempfile::TempDir;
use ui_visual_audit::ComparisonRequest;

pub struct TestRepository {
    _temporary: TempDir,
    pub root: PathBuf,
    pub inputs: PathBuf,
    pub outputs: PathBuf,
}

impl TestRepository {
    pub fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().to_path_buf();
        let inputs = root.join("inputs");
        let outputs = root.join("outputs");
        fs::create_dir_all(&inputs).unwrap();
        fs::create_dir_all(&outputs).unwrap();
        Self {
            _temporary: temporary,
            root,
            inputs,
            outputs,
        }
    }

    pub fn write_png(&self, name: &str, width: u32, height: u32, rgba: &[u8]) -> PathBuf {
        assert_eq!(rgba.len(), width as usize * height as usize * 4);
        let path = self.inputs.join(name);
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(rgba, width, height, ExtendedColorType::Rgba8)
            .unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    #[allow(dead_code)]
    pub fn write_rgb_png(&self, name: &str, width: u32, height: u32, rgb: &[u8]) -> PathBuf {
        assert_eq!(rgb.len(), width as usize * height as usize * 3);
        let path = self.inputs.join(name);
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(rgb, width, height, ExtendedColorType::Rgb8)
            .unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    pub fn write_bytes(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.inputs.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    #[allow(dead_code)]
    pub fn write_config(&self, threshold: f64) -> PathBuf {
        let source = format!(
            "{{\"schema_version\":1,\"algorithm_version\":\"exact_rgba_v1\",\"max_changed_pixel_ratio\":{threshold}}}"
        );
        self.write_bytes("comparison.config.json", source.as_bytes())
    }

    #[allow(dead_code)]
    pub fn request(
        &self,
        reference: PathBuf,
        actual: PathBuf,
        config: PathBuf,
        output_name: &str,
    ) -> ComparisonRequest {
        ComparisonRequest {
            repository_root: self.root.clone(),
            allowed_input_roots: vec![self.inputs.clone()],
            allowed_output_root: self.outputs.clone(),
            reference,
            actual,
            config,
            mask: None,
            output_directory: self.outputs.join(output_name),
        }
    }
}

#[allow(dead_code)]
pub fn decode_hex(source: &str) -> Vec<u8> {
    assert_eq!(source.len() % 2, 0);
    source
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(text, 16).unwrap()
        })
        .collect()
}
