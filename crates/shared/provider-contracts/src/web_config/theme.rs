use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontsConfig {
    pub body: FontConfig,
    pub heading: FontConfig,
    #[serde(default)]
    pub brand: Option<FontConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontConfig {
    pub family: String,
    pub fallback: String,
    #[serde(default)]
    pub files: Vec<FontFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontFile {
    pub path: String,
    pub weight: u16,
    pub style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorsConfig {
    pub light: ColorPalette,
    pub dark: ColorPalette,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPalette {
    pub primary: PrimaryColor,
    pub secondary: PrimaryColor,
    pub success: String,
    pub warning: String,
    pub error: String,
    pub surface: SurfaceColors,
    pub text: TextColors,
    pub background: BackgroundColors,
    pub border: BorderColors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryColor {
    pub hsl: String,
    pub rgb: [u8; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceColors {
    pub default: String,
    pub dark: String,
    pub variant: String,
    #[serde(rename = "secondaryContainer")]
    pub secondary_container: String,
    #[serde(rename = "errorContainer")]
    pub error_container: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextColors {
    pub primary: String,
    pub secondary: String,
    pub inverted: String,
    pub disabled: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundColors {
    pub default: String,
    pub dark: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderColors {
    pub default: String,
    pub dark: String,
    pub outline: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographyConfig {
    pub sizes: TypographySizes,
    pub weights: TypographyWeights,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographySizes {
    pub xs: String,
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub xl: String,
    pub xxl: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TypographyWeights {
    pub regular: u16,
    pub medium: u16,
    pub semibold: u16,
    pub bold: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacingConfig {
    pub xs: String,
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub xl: String,
    pub xxl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadiusConfig {
    pub xs: String,
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub xl: String,
    pub xxl: String,
    pub round: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowsConfig {
    pub light: ShadowSet,
    pub dark: ShadowSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowSet {
    pub sm: String,
    pub md: String,
    pub lg: String,
    pub accent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationConfig {
    pub fast: String,
    pub normal: String,
    pub slow: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ZIndexConfig {
    pub base: u32,
    pub content: u32,
    pub navigation: u32,
    pub modal: u32,
    pub tooltip: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    #[serde(rename = "headerHeight")]
    pub header_height: String,
    #[serde(rename = "sidebarLeft")]
    pub sidebar_left: SidebarConfig,
    #[serde(rename = "sidebarRight")]
    pub sidebar_right: SidebarConfig,
    #[serde(rename = "navHeight")]
    pub nav_height: String,
    #[serde(rename = "contentMaxWidth")]
    pub content_max_width: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarConfig {
    pub width: String,
    #[serde(rename = "minWidth")]
    pub min_width: String,
    #[serde(rename = "maxWidth")]
    pub max_width: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardConfig {
    pub radius: CardRadius,
    pub padding: CardPadding,
    pub gradient: CardGradient,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardRadius {
    pub default: String,
    pub cut: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardPadding {
    pub sm: String,
    pub md: String,
    pub lg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardGradient {
    pub start: String,
    pub mid: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileConfig {
    pub spacing: SpacingConfig,
    pub typography: MobileTypography,
    pub layout: MobileLayout,
    pub card: MobileCardConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileTypography {
    pub sizes: TypographySizes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileLayout {
    #[serde(rename = "headerHeight")]
    pub header_height: String,
    #[serde(rename = "navHeight")]
    pub nav_height: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileCardConfig {
    pub padding: CardPadding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchTargetsConfig {
    pub default: String,
    pub sm: String,
    pub lg: String,
}
