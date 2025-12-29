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
  preferSvg = false,
}) => {
  const logoAlt = alt || theme.branding.name;

  if (preferSvg) {
    return (
      <img
        src="/assets/logos/logo.svg"
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
      <source srcSet="/assets/logos/logo.webp" type="image/webp" />
      <source srcSet="/assets/logos/logo.png" type="image/png" />
      <img
        src="/assets/logos/logo.svg"
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
