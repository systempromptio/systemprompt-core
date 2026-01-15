import { AlertTriangle } from 'lucide-react'
import type { Artifact } from '@/types/artifact'
import { extractVideoData } from '@/lib/artifacts'

export interface VideoHints {
  max_width?: string
  aspect_ratio?: string
  rounded?: boolean
}

interface VideoRendererProps {
  artifact: Artifact
  hints?: VideoHints
}

export function VideoRenderer({ artifact, hints }: VideoRendererProps) {
  const { data, errors } = extractVideoData(artifact)

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
        No video data available
      </div>
    )
  }

  const { src, mime_type, poster, caption, controls, autoplay, loop_playback, muted } = data

  const videoClasses = [
    'max-w-full',
    hints?.rounded !== false ? 'rounded-lg' : '',
  ].filter(Boolean).join(' ')

  const videoStyle: React.CSSProperties = {
    maxWidth: hints?.max_width || '100%',
    aspectRatio: hints?.aspect_ratio,
  }

  return (
    <figure className="artifact-video">
      <video
        src={src}
        poster={poster}
        controls={controls}
        autoPlay={autoplay}
        loop={loop_playback}
        muted={muted || autoplay}
        playsInline
        className={videoClasses}
        style={videoStyle}
      >
        {mime_type && <source src={src} type={mime_type} />}
        Your browser does not support video playback.
      </video>
      {caption && (
        <figcaption className="text-sm text-secondary mt-2 text-center">
          {caption}
        </figcaption>
      )}
    </figure>
  )
}
