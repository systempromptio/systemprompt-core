export interface ThemeBranding {
  name: string;
  title: string;
  description: string;
  themeColor: string;
  logo: {
    primary: string;
    dark: string;
    small: string;
  };
  favicon: string;
}

export interface FontFile {
  path: string;
  weight: number;
  style: 'normal' | 'italic' | 'oblique';
}

export interface FontDefinition {
  family: string;
  fallback: string;
  files: FontFile[];
}

export interface ThemeFonts {
  body: FontDefinition;
  heading: FontDefinition;
  brand: FontDefinition;
}

export interface ColorValue {
  hsl: string;
  rgb: [number, number, number] | undefined;
}

export interface SurfaceColors {
  default: string;
  dark: string;
  variant: string;
  secondaryContainer: string;
  errorContainer: string;
}

export interface TextColors {
  primary: string;
  secondary: string;
  inverted: string;
  disabled: string;
}

export interface BackgroundColors {
  default: string;
  dark: string;
}

export interface BorderColors {
  default: string;
  dark: string;
  outline: string;
}

export interface ColorPalette {
  primary: ColorValue;
  secondary: ColorValue;
  success: string;
  warning: string;
  error: string;
  surface: SurfaceColors;
  text: TextColors;
  background: BackgroundColors;
  border: BorderColors;
}

export interface ThemeColors {
  light: ColorPalette;
  dark: ColorPalette;
}

export interface TypographySizes {
  xs: string;
  sm: string;
  md: string;
  lg: string;
  xl: string;
  xxl: string;
}

export interface TypographyWeights {
  regular: number;
  medium: number;
  semibold: number;
  bold: number;
}

export interface ThemeTypography {
  sizes: TypographySizes;
  weights: TypographyWeights;
}

export interface SpacingScale {
  xs: string;
  sm: string;
  md: string;
  lg: string;
  xl: string;
  xxl: string;
}

export interface RadiusScale {
  xs: string;
  sm: string;
  md: string;
  lg: string;
  xl: string;
  xxl: string;
  round: string;
}

export interface ShadowScale {
  sm: string;
  md: string;
  lg: string;
  accent: string;
}

export interface ThemeShadows {
  light: ShadowScale;
  dark: ShadowScale;
}

export interface ThemeAnimation {
  fast: string;
  normal: string;
  slow: string;
}

export interface ThemeZIndex {
  base: number;
  content: number;
  navigation: number;
  modal: number;
  tooltip: number;
}

export interface SidebarConfig {
  width: string;
  minWidth: string;
  maxWidth: string;
}

export interface ThemeLayout {
  headerHeight: string;
  sidebarLeft: SidebarConfig;
  sidebarRight: SidebarConfig;
  navHeight: string;
  contentMaxWidth: string;
}

export interface CardRadius {
  default: string;
  cut: string;
}

export interface CardPadding {
  sm: string;
  md: string;
  lg: string;
}

export interface CardGradient {
  start: string;
  mid: string;
  end: string;
}

export interface ThemeCard {
  radius: CardRadius;
  padding: CardPadding;
  gradient: CardGradient;
}

export interface MobileTypography {
  sizes: TypographySizes;
}

export interface MobileLayout {
  headerHeight: string;
  navHeight: string;
}

export interface MobileCard {
  padding: CardPadding;
}

export interface ThemeMobile {
  spacing: SpacingScale;
  typography: MobileTypography;
  layout: MobileLayout;
  card: MobileCard;
}

export interface TouchTargets {
  default: string;
  sm: string;
  lg: string;
}

export interface Theme {
  branding: ThemeBranding;
  fonts: ThemeFonts;
  colors: ThemeColors;
  typography: ThemeTypography;
  spacing: SpacingScale;
  radius: RadiusScale;
  shadows: ThemeShadows;
  animation: ThemeAnimation;
  zIndex: ThemeZIndex;
  layout: ThemeLayout;
  card: ThemeCard;
  mobile: ThemeMobile;
  touchTargets: TouchTargets;
}
