import { create } from 'zustand'
import type { AgentCard } from '@a2a-js/sdk'
import type { AgentUrl } from '@/types/core/brand'

interface AgentStore {
  agents: readonly AgentCard[]
  selectedAgentUrl: AgentUrl | undefined
  selectedAgent: AgentCard | undefined
  loading: boolean
  error: string | undefined
  selectionError: string | undefined

  setAgents: (agents: AgentCard[]) => void
  addAgent: (agent: AgentCard) => void
  selectAgent: (agentUrl: AgentUrl, agent: AgentCard) => void
  setLoading: (loading: boolean) => void
  setError: (error: string | undefined) => void
  setSelectionError: (error: string | undefined) => void
  clearSelection: () => void
}

export const useAgentStore = create<AgentStore>()((set) => ({
  agents: [],
  selectedAgentUrl: undefined,
  selectedAgent: undefined,
  loading: false,
  error: undefined,
  selectionError: undefined,

  setAgents: (agents) => set({ agents }),

  addAgent: (agent) =>
    set((state) => ({
      agents: [...state.agents.filter((a) => a.url !== agent.url), agent],
    })),

  selectAgent: (agentUrl, agent) => {
    set({
      selectedAgentUrl: agentUrl,
      selectedAgent: agent,
      selectionError: undefined,
    })
  },

  setLoading: (loading) => set({ loading }),

  setError: (error) => set({ error }),

  setSelectionError: (error) => set({ selectionError: error }),

  clearSelection: () =>
    set({
      selectedAgentUrl: undefined,
      selectedAgent: undefined,
      selectionError: undefined,
    }),
}))

export const agentSelectors = {
  getSelectedAgent: (state: AgentStore): AgentCard | undefined => state.selectedAgent,

  getAgentByUrl: (state: AgentStore, url: AgentUrl): AgentCard | undefined =>
    state.agents.find((a) => a.url === url),

  isLoading: (state: AgentStore): boolean => state.loading,

  hasError: (state: AgentStore): boolean => state.error !== undefined,

  getError: (state: AgentStore): string | undefined => state.error,

  getAgentCount: (state: AgentStore): number => state.agents.length,

  hasAnyAgents: (state: AgentStore): boolean => state.agents.length > 0,

  isAgentSelected: (state: AgentStore): boolean => state.selectedAgent !== undefined,
}
