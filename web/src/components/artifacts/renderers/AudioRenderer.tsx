import { Music, AlertTriangle } from 'lucide-react'
import type { Artifact } from '@/types/artifact'
import { extractAudioData } from '@/lib/artifacts'

export interface AudioHints {
  show_artwork?: boolean
}

interface AudioRendererProps {
  artifact: Artifact
  hints?: AudioHints
}

export function AudioRenderer({ artifact, hints }: AudioRendererProps) {
  const { data, errors } = extractAudioData(artifact)

  if (errors && errors.length > 0) {
    return (
      <div className="flex items-center gap-2 text-error text-sm p-3 bg-error/10 border border-error/20 rounded">
        <AlertTriangle className="w-4 h-4" />
        <div>
          {errors.map((error, idx) => (
            <div key={idx}>{error}</div>
          ))}
        </div>
      </div>
    )
  }

  if (!data) {
    return (
      <div className="text-secondary text-sm italic py-4 text-center">
        No audio data available
      </div>
    )
  }

  const { src, mime_type, title, artist, artwork, controls, autoplay, loop_playback } = data
  const showArtwork = hints?.show_artwork !== false

  return (
    <div className="artifact-audio p-4 border border-primary-10 rounded-lg bg-surface-variant">
      <div className="flex items-start gap-4">
        {showArtwork && (
          <div className="flex-shrink-0 w-16 h-16 rounded-lg bg-surface flex items-center justify-center overflow-hidden">
            {artwork ? (
              <img src={artwork} alt={title || 'Album art'} className="w-full h-full object-cover" />
            ) : (
              <Music className="w-8 h-8 text-secondary" />
            )}
          </div>
        )}
        <div className="flex-1 min-w-0">
          {(title || artist) && (
            <div className="mb-3">
              {title && <div className="font-medium text-primary truncate">{title}</div>}
              {artist && <div className="text-sm text-secondary truncate">{artist}</div>}
            </div>
          )}
          <audio
            src={src}
            controls={controls}
            autoPlay={autoplay}
            loop={loop_playback}
            className="w-full"
          >
            {mime_type && <source src={src} type={mime_type} />}
            Your browser does not support audio playback.
          </audio>
        </div>
      </div>
    </div>
  )
}
