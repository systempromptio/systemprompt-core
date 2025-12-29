import { useEffect, useRef } from 'react'
import { useSearchParams } from 'react-router-dom'
import { useArtifactStore } from '@/stores/artifact.store'
import { useAuth } from './useAuth'
import { createArtifactId } from '@/types/core/brand'
import { logger } from '@/lib/logger'

export function useDeepLink() {
  const [searchParams, setSearchParams] = useSearchParams()
  const { authHeader } = useAuth()
  const { byId, fetchArtifact, openArtifact } = useArtifactStore()
  const processedRef = useRef<string | null>(null)

  useEffect(() => {
    const artifactId = searchParams.get('artifact')

    if (!artifactId || processedRef.current === artifactId) {
      return
    }

    processedRef.current = artifactId

    const handleArtifactDeepLink = async () => {
      if (byId[createArtifactId(artifactId)]) {
        openArtifact(createArtifactId(artifactId))
      } else {
        try {
          await fetchArtifact(createArtifactId(artifactId), authHeader)
          openArtifact(createArtifactId(artifactId))
        } catch (error) {
          logger.error('Failed to fetch artifact for deep link', error, 'useDeepLink')
        }
      }

      const newParams = new URLSearchParams(searchParams)
      newParams.delete('artifact')
      setSearchParams(newParams, { replace: true })
    }

    handleArtifactDeepLink()
  }, [searchParams, authHeader, byId, fetchArtifact, openArtifact, setSearchParams])
}
