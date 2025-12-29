import { create } from 'zustand'
import type { Tool } from '@modelcontextprotocol/sdk/types.js'

export interface McpTool extends Tool {
  serverName: string
  serverEndpoint: string
}

export interface McpServer {
  name: string
  endpoint: string
  auth: string
  status: string
  oauth_required?: boolean
  oauth_scopes?: string[]
}

interface ToolsStore {
  tools: readonly McpTool[]
  servers: readonly McpServer[]
  loading: boolean
  error: string | undefined

  setTools: (tools: McpTool[]) => void
  setServers: (servers: McpServer[]) => void
  setLoading: (loading: boolean) => void
  setError: (error: string | undefined) => void
  clearTools: () => void
}

export const useToolsStore = create<ToolsStore>((set) => ({
  tools: [],
  servers: [],
  loading: false,
  error: undefined,

  setTools: (tools) => set({ tools }),

  setServers: (servers) => set({ servers }),

  setLoading: (loading) => set({ loading }),

  setError: (error) => set({ error }),

  clearTools: () => set({ tools: [], servers: [], error: undefined }),
}))

export const toolsSelectors = {
  getToolByName: (state: ToolsStore, name: string): McpTool | undefined =>
    state.tools.find((tool) => tool.name === name),

  getToolsByServer: (state: ToolsStore, serverName: string): readonly McpTool[] =>
    state.tools.filter((tool) => tool.serverName === serverName),

  getServerByName: (state: ToolsStore, name: string): McpServer | undefined =>
    state.servers.find((server) => server.name === name),

  getServerByEndpoint: (state: ToolsStore, endpoint: string): McpServer | undefined =>
    state.servers.find((server) => server.endpoint === endpoint),

  getToolCount: (state: ToolsStore): number => state.tools.length,

  getServerCount: (state: ToolsStore): number => state.servers.length,

  hasAnyTools: (state: ToolsStore): boolean => state.tools.length > 0,

  hasAnyServers: (state: ToolsStore): boolean => state.servers.length > 0,

  isLoading: (state: ToolsStore): boolean => state.loading,

  hasError: (state: ToolsStore): boolean => state.error !== undefined,

  getError: (state: ToolsStore): string | undefined => state.error,
}
