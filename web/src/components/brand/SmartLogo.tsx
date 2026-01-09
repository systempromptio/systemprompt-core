import React from 'react';
import { theme } from '@/theme.config';

interface SmartLogoProps {
  className?: string;
  width?: number | string;
  height?: number | string;
  alt?: string;
  variant?: 'primary' | 'dark' | 'small';
  preferSvg?: boolean;
}

export const SmartLogo: React.FC<SmartLogoProps> = ({
  className = '',
  width = 200,
  height = 60,
  alt,
  variant = 'primary',
  preferSvg = false,
}) => {
  const logoAlt = alt || theme.branding.name;
  const logoConfig = theme.branding.logo[variant] || theme.branding.logo.primary;

  if (preferSvg && logoConfig.svg) {
    return (
      <img
        src={logoConfig.svg}
        alt={logoAlt}
        width={width}
        height={height}
        className={className}
        style={{ objectFit: 'contain' }}
      />
    );
  }

  return (
    <picture>
      {logoConfig.webp && <source srcSet={logoConfig.webp} type="image/webp" />}
      {logoConfig.svg && <source srcSet={logoConfig.svg} type="image/svg+xml" />}
      <img
        src={logoConfig.png || logoConfig.svg}
        alt={logoAlt}
        width={width}
        height={height}
        className={className}
        style={{ objectFit: 'contain' }}
      />
    </picture>
  );
};

export default SmartLogo;
