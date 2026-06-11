const WINDOW_PROFILE_FLAG: &str = "--window-profile";
const WINDOW_SIZE_FLAG: &str = "--window-size";
const WINDOW_SCALE_FLAG: &str = "--window-scale";
const DEVICE_SCALE_FLAG: &str = "--device-scale";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl WindowSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowProfile {
    Desktop,
    PhonePortrait,
    Phone1080p,
    PhoneSmall,
    TabletPortrait,
    TabletLandscape,
}

impl WindowProfile {
    const fn preset(self) -> WindowDevicePreset {
        match self {
            Self::Desktop => WindowDevicePreset::new(WindowSize::new(1280, 720), 1.0),
            Self::PhonePortrait => WindowDevicePreset::new(WindowSize::new(1280, 2772), 3.25),
            Self::Phone1080p => WindowDevicePreset::new(WindowSize::new(1080, 2400), 3.0),
            Self::PhoneSmall => WindowDevicePreset::new(WindowSize::new(720, 1600), 2.0),
            Self::TabletPortrait => WindowDevicePreset::new(WindowSize::new(1600, 2560), 2.0),
            Self::TabletLandscape => WindowDevicePreset::new(WindowSize::new(2560, 1600), 2.0),
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "desktop" => Some(Self::Desktop),
            "phone-portrait" => Some(Self::PhonePortrait),
            "phone-1080p" => Some(Self::Phone1080p),
            "phone-small" => Some(Self::PhoneSmall),
            "tablet-portrait" => Some(Self::TabletPortrait),
            "tablet-landscape" => Some(Self::TabletLandscape),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct WindowDevicePreset {
    size: WindowSize,
    device_scale: f32,
}

impl WindowDevicePreset {
    const fn new(size: WindowSize, device_scale: f32) -> Self {
        Self { size, device_scale }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WindowStartupConfig {
    pub size: WindowSize,
    pub device_scale: f32,
    pub preview_scale: f32,
    pub warnings: Vec<String>,
}

impl Default for WindowStartupConfig {
    fn default() -> Self {
        let preset = WindowProfile::Desktop.preset();
        Self {
            size: preset.size,
            device_scale: preset.device_scale,
            preview_scale: 1.0,
            warnings: Vec::new(),
        }
    }
}

impl WindowStartupConfig {
    pub fn physical_size(&self) -> WindowSize {
        scaled_window_size(self.size, self.preview_scale)
    }

    pub fn scale_factor_override(&self) -> f32 {
        self.device_scale * self.preview_scale
    }

    pub fn logical_width(&self) -> f32 {
        self.size.width as f32 / self.device_scale
    }

    pub fn logical_height(&self) -> f32 {
        self.size.height as f32 / self.device_scale
    }
}

pub(crate) fn resolve_from_env_args() -> WindowStartupConfig {
    resolve_from_args(std::env::args().skip(1))
}

pub(crate) fn resolve_from_args<I, S>(args: I) -> WindowStartupConfig
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut config = WindowStartupConfig::default();
    let mut args = args.into_iter().peekable();

    while let Some(arg) = args.next() {
        let arg = arg.as_ref();

        if arg == WINDOW_PROFILE_FLAG {
            if let Some(value) =
                next_flag_value(&mut args, WINDOW_PROFILE_FLAG, &mut config.warnings)
            {
                apply_profile(&value, &mut config);
            }
        } else if let Some(value) = arg.strip_prefix("--window-profile=") {
            apply_profile(value, &mut config);
        } else if arg == WINDOW_SIZE_FLAG {
            if let Some(value) = next_flag_value(&mut args, WINDOW_SIZE_FLAG, &mut config.warnings)
            {
                apply_size(&value, &mut config);
            }
        } else if let Some(value) = arg.strip_prefix("--window-size=") {
            apply_size(value, &mut config);
        } else if arg == WINDOW_SCALE_FLAG {
            if let Some(value) = next_flag_value(&mut args, WINDOW_SCALE_FLAG, &mut config.warnings)
            {
                apply_preview_scale(&value, &mut config);
            }
        } else if let Some(value) = arg.strip_prefix("--window-scale=") {
            apply_preview_scale(value, &mut config);
        } else if arg == DEVICE_SCALE_FLAG {
            if let Some(value) = next_flag_value(&mut args, DEVICE_SCALE_FLAG, &mut config.warnings)
            {
                apply_device_scale(&value, &mut config);
            }
        } else if let Some(value) = arg.strip_prefix("--device-scale=") {
            apply_device_scale(value, &mut config);
        }
    }

    config
}

fn next_flag_value<I, S>(
    args: &mut std::iter::Peekable<I>,
    flag: &str,
    warnings: &mut Vec<String>,
) -> Option<String>
where
    I: Iterator<Item = S>,
    S: AsRef<str>,
{
    match args.peek() {
        Some(next) if next.as_ref().starts_with("--") => {
            warnings.push(format!(
                "{flag} requires a value; keeping current window size"
            ));
            None
        }
        Some(_) => args.next().map(|value| value.as_ref().to_owned()),
        None => {
            warnings.push(format!(
                "{flag} requires a value; keeping current window size"
            ));
            None
        }
    }
}

fn apply_profile(value: &str, config: &mut WindowStartupConfig) {
    if let Some(profile) = WindowProfile::parse(value) {
        let preset = profile.preset();
        config.size = preset.size;
        config.device_scale = preset.device_scale;
    } else {
        let preset = WindowProfile::Desktop.preset();
        config.size = preset.size;
        config.device_scale = preset.device_scale;
        config.warnings.push(format!(
            "unknown window profile '{value}'; using default window size"
        ));
    }
}

fn apply_size(value: &str, config: &mut WindowStartupConfig) {
    if let Some(size) = parse_window_size(value) {
        config.size = size;
        if let Some(device_scale) = inferred_device_scale(size) {
            config.device_scale = device_scale;
        }
    } else {
        let preset = WindowProfile::Desktop.preset();
        config.size = preset.size;
        config.device_scale = preset.device_scale;
        config.warnings.push(format!(
            "invalid window size '{value}'; expected WIDTHxHEIGHT, using default window size"
        ));
    }
}

fn apply_preview_scale(value: &str, config: &mut WindowStartupConfig) {
    if let Some(scale) = parse_positive_scale(value) {
        config.preview_scale = scale;
    } else {
        config.preview_scale = 1.0;
        config.warnings.push(format!(
            "invalid window scale '{value}'; expected a positive number or percentage, using 100% preview scale"
        ));
    }
}

fn apply_device_scale(value: &str, config: &mut WindowStartupConfig) {
    if let Some(scale) = parse_positive_scale(value) {
        config.device_scale = scale;
    } else {
        config.device_scale = WindowProfile::Desktop.preset().device_scale;
        config.warnings.push(format!(
            "invalid device scale '{value}'; expected a positive number or percentage, using 1.0"
        ));
    }
}

fn parse_window_size(value: &str) -> Option<WindowSize> {
    let value = value.trim();
    let (width, height) = value.split_once('x').or_else(|| value.split_once('X'))?;
    let width = width.trim().parse::<u32>().ok()?;
    let height = height.trim().parse::<u32>().ok()?;

    if width == 0 || height == 0 {
        return None;
    }

    Some(WindowSize::new(width, height))
}

fn parse_positive_scale(value: &str) -> Option<f32> {
    let value = value.trim();
    let scale = if let Some(percent) = value.strip_suffix('%') {
        percent.trim().parse::<f32>().ok()? / 100.0
    } else {
        value.parse::<f32>().ok()?
    };

    (scale.is_finite() && scale > 0.0).then_some(scale)
}

fn inferred_device_scale(size: WindowSize) -> Option<f32> {
    [
        WindowProfile::Desktop,
        WindowProfile::PhonePortrait,
        WindowProfile::Phone1080p,
        WindowProfile::PhoneSmall,
        WindowProfile::TabletPortrait,
        WindowProfile::TabletLandscape,
    ]
    .into_iter()
    .map(WindowProfile::preset)
    .find(|preset| preset.size == size)
    .map(|preset| preset.device_scale)
}

fn scaled_window_size(size: WindowSize, scale: f32) -> WindowSize {
    WindowSize::new(
        ((size.width as f32) * scale).round().max(1.0) as u32,
        ((size.height as f32) * scale).round().max(1.0) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.0001;

    fn assert_approx_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= EPSILON,
            "expected {actual} to be approximately {expected}"
        );
    }

    #[test]
    fn parses_window_profiles() {
        assert_eq!(
            WindowProfile::parse("phone-portrait").map(|profile| profile.preset().size),
            Some(WindowSize::new(1280, 2772))
        );
        assert_eq!(
            WindowProfile::parse("phone-1080p").map(|profile| profile.preset().size),
            Some(WindowSize::new(1080, 2400))
        );
        assert_eq!(
            WindowProfile::parse("phone-small").map(|profile| profile.preset().size),
            Some(WindowSize::new(720, 1600))
        );
        assert_eq!(
            WindowProfile::parse("tablet-portrait").map(|profile| profile.preset().size),
            Some(WindowSize::new(1600, 2560))
        );
        assert_eq!(
            WindowProfile::parse("tablet-landscape").map(|profile| profile.preset().size),
            Some(WindowSize::new(2560, 1600))
        );
        assert_eq!(
            WindowProfile::parse("desktop").map(|profile| profile.preset().size),
            Some(WindowSize::new(1280, 720))
        );
        assert_eq!(WindowProfile::parse("unknown"), None);
    }

    #[test]
    fn resolves_profile_arguments() {
        let config = resolve_from_args([WINDOW_PROFILE_FLAG, "phone-small"]);

        assert_eq!(config.size, WindowSize::new(720, 1600));
        assert_approx_eq(config.device_scale, 2.0);
        assert_approx_eq(config.logical_width(), 360.0);
        assert_approx_eq(config.logical_height(), 800.0);
        assert!(config.warnings.is_empty());
    }

    #[test]
    fn resolves_custom_size_arguments() {
        let config = resolve_from_args([WINDOW_SIZE_FLAG, "1280x2772"]);

        assert_eq!(config.size, WindowSize::new(1280, 2772));
        assert_approx_eq(config.device_scale, 3.25);
        assert_approx_eq(config.preview_scale, 1.0);
        assert!(config.warnings.is_empty());
    }

    #[test]
    fn resolves_equals_style_arguments() {
        let config = resolve_from_args([
            "--window-profile=tablet-landscape",
            "--window-size=900x1200",
            "--window-scale=50%",
        ]);

        assert_eq!(config.size, WindowSize::new(900, 1200));
        assert_approx_eq(config.preview_scale, 0.5);
        assert_eq!(config.physical_size(), WindowSize::new(450, 600));
        assert!(config.warnings.is_empty());
    }

    #[test]
    fn resolves_preview_scale_arguments() {
        let decimal = resolve_from_args([
            WINDOW_PROFILE_FLAG,
            "phone-portrait",
            WINDOW_SCALE_FLAG,
            "0.5",
        ]);
        assert_eq!(decimal.size, WindowSize::new(1280, 2772));
        assert_approx_eq(decimal.device_scale, 3.25);
        assert_approx_eq(decimal.preview_scale, 0.5);
        assert_eq!(decimal.physical_size(), WindowSize::new(640, 1386));
        assert_approx_eq(decimal.scale_factor_override(), 1.625);
        assert_approx_eq(decimal.logical_width(), 393.84616);
        assert_approx_eq(decimal.logical_height(), 852.9231);

        let percent = resolve_from_args([WINDOW_SCALE_FLAG, "75%"]);
        assert_approx_eq(percent.preview_scale, 0.75);
        assert_eq!(percent.physical_size(), WindowSize::new(960, 540));
    }

    #[test]
    fn resolves_device_scale_arguments() {
        let config = resolve_from_args([WINDOW_SIZE_FLAG, "1280x2772", DEVICE_SCALE_FLAG, "2.5"]);

        assert_eq!(config.size, WindowSize::new(1280, 2772));
        assert_approx_eq(config.device_scale, 2.5);
        assert_approx_eq(config.logical_width(), 512.0);
        assert_approx_eq(config.logical_height(), 1108.8);
    }

    #[test]
    fn falls_back_for_unknown_profile() {
        let config = resolve_from_args([WINDOW_PROFILE_FLAG, "unknown-phone"]);

        assert_eq!(config.size, WindowProfile::Desktop.preset().size);
        assert_eq!(config.warnings.len(), 1);
    }

    #[test]
    fn falls_back_for_invalid_size() {
        for args in [
            [WINDOW_SIZE_FLAG, "1280"],
            [WINDOW_SIZE_FLAG, "1280x0"],
            [WINDOW_SIZE_FLAG, "0x720"],
            [WINDOW_SIZE_FLAG, "widextall"],
        ] {
            let config = resolve_from_args(args);

            assert_eq!(config.size, WindowProfile::Desktop.preset().size);
            assert_eq!(config.warnings.len(), 1);
        }
    }

    #[test]
    fn falls_back_for_invalid_scale() {
        for args in [
            [WINDOW_SCALE_FLAG, "0"],
            [WINDOW_SCALE_FLAG, "-1"],
            [WINDOW_SCALE_FLAG, "NaN"],
            [WINDOW_SCALE_FLAG, "large"],
        ] {
            let config = resolve_from_args(args);

            assert_approx_eq(config.preview_scale, 1.0);
            assert_eq!(config.warnings.len(), 1);
        }
    }

    #[test]
    fn falls_back_for_invalid_device_scale() {
        for args in [
            [DEVICE_SCALE_FLAG, "0"],
            [DEVICE_SCALE_FLAG, "-1"],
            [DEVICE_SCALE_FLAG, "NaN"],
            [DEVICE_SCALE_FLAG, "large"],
        ] {
            let config = resolve_from_args(args);

            assert_approx_eq(config.device_scale, 1.0);
            assert_eq!(config.warnings.len(), 1);
        }
    }

    #[test]
    fn missing_values_do_not_consume_the_next_flag() {
        let config = resolve_from_args([WINDOW_PROFILE_FLAG, WINDOW_SIZE_FLAG, "720x1600"]);

        assert_eq!(config.size, WindowSize::new(720, 1600));
        assert_eq!(config.warnings.len(), 1);
    }
}
