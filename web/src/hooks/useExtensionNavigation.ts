import { theme } from '@/theme.config';
import { useAuthStore } from '@/stores/auth.store';

interface SidebarLink {
  label: string;
  path: string;
  icon?: string;
  external?: boolean;
  auth_required?: boolean;
}

interface SidebarSection {
  id: string;
  title: string;
  priority?: number;
  links: SidebarLink[];
}

/**
 * Hook to get sidebar sections contributed by extensions.
 * Filters links based on authentication state.
 */
export function useExtensionSidebar(): SidebarSection[] {
  const isAuthenticated = useAuthStore((state) => state.isAuthenticated());

  const sections = theme.extensionUi?.sidebar?.sections ?? [];

  return sections
    .map((section) => ({
      ...section,
      links: section.links.filter(
        (link) => !link.auth_required || isAuthenticated
      ),
    }))
    .filter((section) => section.links.length > 0);
}
