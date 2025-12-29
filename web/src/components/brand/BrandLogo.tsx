import React from 'react';
import { theme } from '@/theme.config';

type LogoVariant = 'primary' | 'dark' | 'small'
type LogoFormat = 'svg' | 'webp' | 'png'

interface LogoConfig {
  svg?: string
  webp?: string
  png: string
}

interface BrandLogoProps {
  className?: string;
  width?: number | string;
  height?: number | string;
  alt?: string;
  variant?: LogoVariant;
  format?: LogoFormat;
}

export const BrandLogo: React.FC<BrandLogoProps> = ({
  className = '',
  width = 200,
  height = 60,
  alt,
  variant = 'primary',
  format = 'svg',
}) => {
  const logoAlt = alt || theme.branding.name;

  const getLogoSrc = (): string => {
    const variantConfig = theme.branding.logo[variant] as LogoConfig | undefined;

    if (variantConfig && format in variantConfig) {
      const formatValue = variantConfig[format as keyof LogoConfig];
      if (formatValue) return formatValue;
    }

    if (variantConfig?.png) {
      return variantConfig.png;
    }

    return theme.branding.logo.primary.png;
  };

  const logoSrc = getLogoSrc();

  return (
    <img
      src={logoSrc}
      alt={logoAlt}
      width={width}
      height={height}
      className={className}
      style={{ objectFit: 'contain' }}
    />
  );
};

export default BrandLogo;
