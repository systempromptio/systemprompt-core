import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export interface SettingsStore {
  debugMode: boolean
  setDebugMode: (enabled: boolean) => void
  leftSidebarVisible: boolean
  toggleLeftSidebar: () => void
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set) => ({
      debugMode: false,

      setDebugMode: (enabled) => set({ debugMode: enabled }),

      leftSidebarVisible: false,

      toggleLeftSidebar: () => set((state) => ({ leftSidebarVisible: !state.leftSidebarVisible })),
    }),
    {
      name: 'systemprompt-settings-v2',
    }
  )
)

export const settingsSelectors = {
  isDebugModeEnabled: (state: SettingsStore): boolean => state.debugMode,

  isLeftSidebarVisible: (state: SettingsStore): boolean => state.leftSidebarVisible,
}
