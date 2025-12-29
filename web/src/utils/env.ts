export function getApiBaseUrl(): string {
  if (import.meta.env.DEV) {
    return '';
  }

  return import.meta.env.VITE_API_BASE_HOST || window.location.origin;
}

export function getApiUrl(path: string): string {
  const baseUrl = getApiBaseUrl();
  const cleanPath = path.startsWith('/') ? path : `/${path}`;
  return `${baseUrl}${cleanPath}`;
}
