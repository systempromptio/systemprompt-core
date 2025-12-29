import { create } from 'zustand'

export type ViewType = 'conversation' | 'tasks' | 'artifacts'

interface ViewStore {
  activeView: ViewType
  setActiveView: (view: ViewType) => void
}

export const useViewStore = create<ViewStore>()((set) => ({
  activeView: 'conversation',

  setActiveView: (view) => set({ activeView: view }),
}))

export const viewSelectors = {
  getActiveView: (state: ViewStore): ViewType => state.activeView,

  isConversationView: (state: ViewStore): boolean => state.activeView === 'conversation',

  isTasksView: (state: ViewStore): boolean => state.activeView === 'tasks',

  isArtifactsView: (state: ViewStore): boolean => state.activeView === 'artifacts',
}
