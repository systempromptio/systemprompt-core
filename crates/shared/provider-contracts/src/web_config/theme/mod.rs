//! Design-token types: fonts, colours, typography, spacing, layout, etc.
//!
//! All types here are pure data containers serialized to / from YAML.

mod card;
mod colors;
mod fonts;
mod layout;
mod mobile;
mod scale;
mod tokens;
mod typography;

pub use card::{CardConfig, CardGradient, CardPadding, CardRadius};
pub use colors::{
    BackgroundColors, BorderColors, ColorPalette, ColorsConfig, PrimaryColor, SurfaceColors,
    TextColors,
};
pub use fonts::{FontConfig, FontFile, FontsConfig};
pub use layout::{LayoutConfig, SidebarConfig};
pub use mobile::{MobileCardConfig, MobileConfig, MobileLayout, MobileTypography};
pub use scale::{RadiusConfig, SpacingConfig};
pub use tokens::{AnimationConfig, ShadowSet, ShadowsConfig, TouchTargetsConfig, ZIndexConfig};
pub use typography::{TypographyConfig, TypographySizes, TypographyWeights};
