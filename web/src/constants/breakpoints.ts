const BREAKPOINTS = {
  mobile: 768,
  tablet: 1024,
  desktop: 1280,
  widescreen: 1536,
} as const

type Breakpoint = keyof typeof BREAKPOINTS
type BreakpointValue = (typeof BREAKPOINTS)[Breakpoint]

function isMobile(width: number): boolean {
  return width < BREAKPOINTS.mobile
}

function isTablet(width: number): boolean {
  return width >= BREAKPOINTS.mobile && width < BREAKPOINTS.desktop
}

function isDesktop(width: number): boolean {
  return width >= BREAKPOINTS.desktop
}

function getBreakpoint(width: number): Breakpoint {
  if (width < BREAKPOINTS.mobile) return 'mobile'
  if (width < BREAKPOINTS.tablet) return 'tablet'
  if (width < BREAKPOINTS.desktop) return 'desktop'
  return 'widescreen'
}

export { BREAKPOINTS, isMobile, isTablet, isDesktop, getBreakpoint }
export type { Breakpoint, BreakpointValue }
