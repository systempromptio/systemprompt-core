import { create } from 'zustand'
import type { Artifact, ArtifactType } from '@/types/artifact'
import { isPersistedArtifact } from '@/types/artifact'
import { artifactsService } from '@/services/artifacts.service'
import { shouldReplaceItem } from '@/utils/store-helpers'
import { ensureInArray, addToMapping } from './store-utilities'
import { extractAndStoreSkill } from '@/lib/utils/extractArtifactSkills'
import {
  type ArtifactId,
  type ContextId,
  type TaskId,
  createArtifactId,
  createContextId,
  createTaskId,
} from '@/types/core/brand'

type IndexKey = 'byTask' | 'byContext'

interface NormalizeOptions {
  indexKey?: IndexKey
  indexValue?: ContextId | TaskId
}

interface ArtifactState {
  byId: Record<ArtifactId, Artifact>
  allIds: readonly ArtifactId[]
  byContext: Record<ContextId, readonly ArtifactId[]>
  byTask: Record<TaskId, readonly ArtifactId[]>
  isLoading: boolean
  error?: string
}

function normalizeArtifacts(
  state: ArtifactState,
  artifacts: Artifact[],
  options: NormalizeOptions = {}
): Partial<ArtifactState> {
  const newById: Record<ArtifactId, Artifact> = { ...state.byId }
  const newAllIds: ArtifactId[] = [...state.allIds]
  const newByContext: Record<ContextId, readonly ArtifactId[]> = { ...state.byContext }
  const newByTask: Record<TaskId, readonly ArtifactId[]> = { ...state.byTask }
  const artifactIds: ArtifactId[] = []

  artifacts.forEach((artifact) => {
    const artifactId = createArtifactId(artifact.artifactId)
    newById[artifactId] = artifact
    artifactIds.push(artifactId)
    if (!newAllIds.includes(artifactId)) {
      newAllIds.push(artifactId)
    }

    if (isPersistedArtifact(artifact)) {
      const contextIdStr = artifact.metadata.context_id
      const taskIdStr = artifact.metadata.task_id

      if (contextIdStr) {
        const contextId = createContextId(contextIdStr)
        addToMapping(newByContext, contextId, artifactId)
      }
      if (taskIdStr) {
        const taskId = createTaskId(taskIdStr)
        addToMapping(newByTask, taskId, artifactId)
        if (contextIdStr) {
          extractAndStoreSkill(artifact, contextIdStr, taskIdStr)
        }
      }
    }
  })

  const result: Partial<ArtifactState> = {
    byId: newById,
    allIds: newAllIds,
    byContext: newByContext,
    byTask: newByTask,
    isLoading: false,
  }

  if (options.indexKey && options.indexValue) {
    if (options.indexKey === 'byContext') {
      const contextId = options.indexValue
      result.byContext = { ...newByContext, [contextId]: artifactIds }
    } else if (options.indexKey === 'byTask') {
      const taskId = options.indexValue
      result.byTask = { ...newByTask, [taskId]: artifactIds }
    }
  }

  return result
}

interface ArtifactStore {
  byId: Record<ArtifactId, Artifact>
  allIds: readonly ArtifactId[]
  byContext: Readonly<Record<ContextId, readonly ArtifactId[]>>
  byTask: Readonly<Record<TaskId, readonly ArtifactId[]>>
  isLoading: boolean
  error: string | undefined
  selectedArtifactId: ArtifactId | undefined
  selectedArtifactIds: readonly ArtifactId[]
  currentArtifactIndex: number

  fetchAllArtifacts: (authToken: string | undefined, limit?: number) => Promise<void>
  fetchArtifactsByContext: (contextId: ContextId, authToken: string | undefined) => Promise<void>
  fetchArtifactsByTask: (taskId: TaskId, authToken: string | undefined) => Promise<void>
  fetchArtifact: (artifactId: ArtifactId, authToken: string | undefined) => Promise<void>
  addArtifact: (artifact: Artifact, taskId?: TaskId, contextId?: ContextId) => void
  clearError: () => void
  getArtifactsByContext: (contextId: ContextId) => Artifact[]
  getArtifactsByTask: (taskId: TaskId) => Artifact[]
  getArtifactsByType: (type: ArtifactType) => Artifact[]
  reset: () => void
  openArtifact: (artifactId: ArtifactId) => void
  openArtifacts: (artifactIds: ArtifactId[]) => void
  nextArtifact: () => void
  previousArtifact: () => void
  closeArtifact: () => void
}

export const useArtifactStore = create<ArtifactStore>()((set, get) => ({
  byId: {},
  allIds: [],
  byContext: {},
  byTask: {},
  isLoading: false,
  error: undefined,
  selectedArtifactId: undefined,
  selectedArtifactIds: [],
  currentArtifactIndex: 0,

  fetchAllArtifacts: async (authToken, limit?) => {
    set({ isLoading: true, error: undefined })
    const result = await artifactsService.listArtifacts(authToken, limit)

    if (!result.ok) {
      const errorMessage = 'message' in result.error
        ? result.error.message
        : `Error: ${result.error.kind}`
      set({ isLoading: false, error: errorMessage })
      return
    }

    const artifacts = [...result.value]
    set((state) => normalizeArtifacts(state, artifacts))
  },

  fetchArtifactsByContext: async (contextId, authToken) => {
    set({ isLoading: true, error: undefined })
    const result = await artifactsService.listArtifactsByContext(contextId, authToken)

    if (!result.ok) {
      const errorMessage = 'message' in result.error
        ? result.error.message
        : `Error: ${result.error.kind}`
      set({ isLoading: false, error: errorMessage })
      return
    }

    const artifacts = [...result.value]
    set((state) => normalizeArtifacts(state, artifacts, { indexKey: 'byContext', indexValue: contextId }))
  },

  fetchArtifactsByTask: async (taskId, authToken) => {
    set({ isLoading: true, error: undefined })
    const result = await artifactsService.listArtifactsByTask(taskId, authToken)

    if (!result.ok) {
      const errorMessage = 'message' in result.error
        ? result.error.message
        : `Error: ${result.error.kind}`
      set({ isLoading: false, error: errorMessage })
      return
    }

    const artifacts = [...result.value]
    set((state) => normalizeArtifacts(state, artifacts, { indexKey: 'byTask', indexValue: taskId }))
  },

  fetchArtifact: async (artifactId, authToken) => {
    set({ isLoading: true, error: undefined })
    const result = await artifactsService.getArtifact(artifactId, authToken)

    if (!result.ok) {
      const errorMessage = result.error.kind === 'network' && result.error.status === 404
        ? `Artifact ${artifactId} not found`
        : 'message' in result.error
          ? result.error.message
          : `Error: ${result.error.kind}`
      set({ isLoading: false, error: errorMessage })
      return
    }

    set((state) => normalizeArtifacts(state, [result.value]))
  },

  addArtifact: (artifact, taskId?, contextId?) => {
    set((state) => {
      const artifactId = createArtifactId(artifact.artifactId)
      const existing = state.byId[artifactId]

      if (!shouldReplaceItem(artifact.metadata, existing?.metadata)) {
        return state
      }

      const newByContext: Record<ContextId, readonly ArtifactId[]> = { ...state.byContext }
      const newByTask: Record<TaskId, readonly ArtifactId[]> = { ...state.byTask }

      if (contextId) {
        addToMapping(newByContext, contextId, artifactId)
      }

      if (taskId) {
        addToMapping(newByTask, taskId, artifactId)
      }

      return {
        byId: { ...state.byId, [artifactId]: artifact },
        allIds: ensureInArray(artifactId, state.allIds),
        byContext: newByContext,
        byTask: newByTask,
      }
    })
  },

  clearError: () => set({ error: undefined }),

  getArtifactsByContext: (contextId) => {
    const state = get()
    const artifactIds = state.byContext[contextId] || []
    return artifactIds
      .map((id) => state.byId[id])
      .filter((artifact): artifact is Artifact => artifact !== undefined)
  },

  getArtifactsByTask: (taskId) => {
    const state = get()
    const artifactIds = state.byTask[taskId] || []
    return artifactIds
      .map((id) => state.byId[id])
      .filter((artifact): artifact is Artifact => artifact !== undefined)
  },

  getArtifactsByType: (type) => {
    const state = get()
    return state.allIds
      .map(id => state.byId[id])
      .filter((artifact) => artifact.metadata.artifact_type === type)
  },

  reset: () => {
    set({
      byId: {},
      allIds: [],
      byContext: {},
      byTask: {},
      isLoading: false,
      error: undefined,
      selectedArtifactId: undefined,
      selectedArtifactIds: [],
      currentArtifactIndex: 0,
    })
  },

  openArtifact: (artifactId: ArtifactId) => {
    set({
      selectedArtifactId: artifactId,
      selectedArtifactIds: [artifactId],
      currentArtifactIndex: 0,
    })
  },

  openArtifacts: (artifactIds: ArtifactId[]) => {
    if (artifactIds.length === 0) return
    set({
      selectedArtifactId: artifactIds[0],
      selectedArtifactIds: artifactIds,
      currentArtifactIndex: 0,
    })
  },

  nextArtifact: () => {
    const state = get()
    if (state.selectedArtifactIds.length === 0) return
    const nextIndex = (state.currentArtifactIndex + 1) % state.selectedArtifactIds.length
    const nextArtifactId = state.selectedArtifactIds[nextIndex]
    set({
      selectedArtifactId: nextArtifactId,
      currentArtifactIndex: nextIndex,
    })
  },

  previousArtifact: () => {
    const state = get()
    if (state.selectedArtifactIds.length === 0) return
    const prevIndex = (state.currentArtifactIndex - 1 + state.selectedArtifactIds.length) % state.selectedArtifactIds.length
    const prevArtifactId = state.selectedArtifactIds[prevIndex]
    set({
      selectedArtifactId: prevArtifactId,
      currentArtifactIndex: prevIndex,
    })
  },

  closeArtifact: () => {
    set({
      selectedArtifactId: undefined,
      selectedArtifactIds: [],
      currentArtifactIndex: 0,
    })
  },
}))

export const artifactSelectors = {
  getArtifactById: (state: ArtifactStore, id: ArtifactId): Artifact | undefined =>
    state.byId[id],

  getSelectedArtifact: (state: ArtifactStore): Artifact | undefined => {
    const { selectedArtifactId, byId } = state
    return selectedArtifactId && byId[selectedArtifactId] ? byId[selectedArtifactId] : undefined
  },

  getArtifactsByContextIds: (state: ArtifactStore, contextId: ContextId): readonly ArtifactId[] =>
    state.byContext[contextId] ?? [],

  getArtifactsByTaskIds: (state: ArtifactStore, taskId: TaskId): readonly ArtifactId[] =>
    state.byTask[taskId] ?? [],

  getArtifactCount: (state: ArtifactStore): number => state.allIds.length,

  isLoading: (state: ArtifactStore): boolean => state.isLoading,

  hasError: (state: ArtifactStore): boolean => state.error !== undefined,

  getError: (state: ArtifactStore): string | undefined => state.error,
}
