import { AlertTriangle } from 'lucide-react'
import type { Artifact } from '@/types/artifact'
import { extractImageData } from '@/lib/artifacts'

export interface ImageHints {
  max_width?: string
  object_fit?: 'contain' | 'cover' | 'fill' | 'none'
  rounded?: boolean
  shadow?: boolean
}

interface ImageRendererProps {
  artifact: Artifact
  hints?: ImageHints
}

export function ImageRenderer({ artifact, hints }: ImageRendererProps) {
  const { data, errors } = extractImageData(artifact)

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
        No image data available
      </div>
    )
  }

  const { src, alt, caption, width, height } = data

  const imageClasses = [
    'max-w-full',
    'h-auto',
    hints?.rounded !== false ? 'rounded-lg' : '',
    hints?.shadow ? 'shadow-lg' : '',
  ].filter(Boolean).join(' ')

  const imageStyle: React.CSSProperties = {
    maxWidth: hints?.max_width || '100%',
    objectFit: hints?.object_fit || 'contain',
  }

  return (
    <figure className="artifact-image">
      <img
        src={src}
        alt={alt || ''}
        width={width}
        height={height}
        className={imageClasses}
        style={imageStyle}
        loading="lazy"
      />
      {caption && (
        <figcaption className="text-sm text-secondary mt-2 text-center">
          {caption}
        </figcaption>
      )}
    </figure>
  )
}
