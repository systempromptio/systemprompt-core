import { useState, useEffect, useRef, type ReactElement } from 'react'
import { ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils/cn'

const morphingStyles = `
@keyframes morph {
  0%, 100% {
    border-radius: 50%;
    transform: rotate(0deg) scale(1);
  }
  25% {
    border-radius: 30%;
    transform: rotate(90deg) scale(0.9);
  }
  50% {
    border-radius: 50% 0 50% 0;
    transform: rotate(180deg) scale(1);
  }
  75% {
    border-radius: 30%;
    transform: rotate(270deg) scale(0.9);
  }
}
@keyframes colorShift {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.7; }
}
`

export function MorphingShape() {
  return (
    <>
      <style>{morphingStyles}</style>
      <div
        className="w-6 h-6 bg-primary"
        style={{
          animation: 'morph 3s ease-in-out infinite, colorShift 2s ease-in-out infinite',
        }}
      />
    </>
  )
}

// eslint-disable-next-line react-refresh/only-export-components
export function useElapsedTime(startedAt: string | undefined, isRunning: boolean): string {
  const [elapsed, setElapsed] = useState('')

  useEffect(() => {
    if (!startedAt || !isRunning) {
      setElapsed('')
      return
    }

    const formatElapsedTime = (start: string): string => {
      const elapsedSecs = (Date.now() - new Date(start).getTime()) / 1000
      if (elapsedSecs < 1) return '0s'
      if (elapsedSecs < 60) return `${Math.floor(elapsedSecs)}s`
      return `${Math.floor(elapsedSecs / 60)}m ${Math.floor(elapsedSecs % 60)}s`
    }

    const update = () => setElapsed(formatElapsedTime(startedAt))
    update()
    const interval = setInterval(update, 1000)
    return () => clearInterval(interval)
  }, [startedAt, isRunning])

  return elapsed
}

interface DetailSectionProps {
  icon: ReactElement
  label: string
  children: React.ReactNode
  collapsible?: boolean
  defaultOpen?: boolean
}

export function DetailSection({ icon, label, children, collapsible = false, defaultOpen = true }: DetailSectionProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen)

  if (!collapsible) {
    return (
      <div>
        <div className="flex items-center gap-1.5 text-text-secondary text-[11px] font-semibold uppercase tracking-wide mb-1.5">
          <span className="text-text-tertiary">{icon}</span>
          {label}
        </div>
        <div className="pl-5">{children}</div>
      </div>
    )
  }

  return (
    <div className="border border-border rounded overflow-hidden">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className="w-full flex items-center gap-1.5 px-2.5 py-2 bg-muted/30 hover:bg-muted/50 transition-colors text-left"
      >
        <ChevronRight className={cn('w-3 h-3 text-text-tertiary transition-transform', isOpen && 'rotate-90')} />
        <span className="text-text-tertiary">{icon}</span>
        <span className="text-[11px] font-semibold uppercase tracking-wide text-text-secondary">{label}</span>
      </button>
      {isOpen && <div className="p-2.5 bg-surface">{children}</div>}
    </div>
  )
}

export function ExpandableJson({ data }: { data: unknown }) {
  const [isExpanded, setIsExpanded] = useState(false)
  const contentRef = useRef<HTMLPreElement>(null)
  const [needsExpand, setNeedsExpand] = useState(false)
  const json = typeof data === 'string' ? data : JSON.stringify(data, null, 2)

  useEffect(() => {
    if (contentRef.current) {
      setNeedsExpand(contentRef.current.scrollHeight > 96)
    }
  }, [json])

  return (
    <div className="relative">
      <pre
        ref={contentRef}
        className={cn(
          'text-[11px] font-mono bg-muted/30 p-2 rounded overflow-auto whitespace-pre-wrap cursor-pointer transition-all duration-200',
          isExpanded ? 'max-h-[600px]' : 'max-h-24'
        )}
        onClick={() => setIsExpanded(!isExpanded)}
      >
        {json}
      </pre>
      {!isExpanded && needsExpand && (
        <div className="absolute bottom-0 left-0 right-0 h-8 bg-gradient-to-t from-muted/90 via-muted/60 to-transparent pointer-events-none flex items-end justify-center pb-1 rounded-b">
          <span className="text-[9px] text-text-tertiary uppercase tracking-wide">Click to expand</span>
        </div>
      )}
      {isExpanded && needsExpand && (
        <div className="absolute bottom-0 left-0 right-0 flex justify-center pb-1 pointer-events-none">
          <span className="text-[9px] text-text-tertiary uppercase tracking-wide pointer-events-auto cursor-pointer hover:text-text-secondary" onClick={(e) => { e.stopPropagation(); setIsExpanded(false); }}>Click to collapse</span>
        </div>
      )}
    </div>
  )
}

export function ExpandableToolArgs({ args }: { args: Record<string, unknown> }) {
  const [isExpanded, setIsExpanded] = useState(false)
  const entries = Object.entries(args)
  const hasMore = entries.length > 3

  const formatValue = (value: unknown, expanded: boolean): string => {
    if (typeof value === 'string') {
      if (expanded) return `"${value}"`
      return value.length > 50 ? `"${value.slice(0, 50)}..."` : `"${value}"`
    }
    if (typeof value === 'boolean' || typeof value === 'number') {
      return String(value)
    }
    if (Array.isArray(value)) {
      if (expanded) return JSON.stringify(value, null, 2)
      return `[${value.length} items]`
    }
    if (value && typeof value === 'object') {
      if (expanded) return JSON.stringify(value, null, 2)
      return '{...}'
    }
    return String(value)
  }

  const displayEntries = isExpanded ? entries : entries.slice(0, 3)

  return (
    <div
      className={cn(
        'text-[11px] font-mono text-text-tertiary bg-surface/50 rounded px-2 py-1 cursor-pointer transition-all duration-200',
        isExpanded ? 'max-h-[400px] overflow-auto' : 'max-h-20 overflow-hidden'
      )}
      onClick={() => setIsExpanded(!isExpanded)}
    >
      <div className="space-y-0.5">
        {displayEntries.map(([key, value]) => (
          <div key={key} className={isExpanded ? 'whitespace-pre-wrap' : 'truncate'}>
            <span className="text-primary/70">{key}:</span>{' '}
            <span className="text-text-secondary">{formatValue(value, isExpanded)}</span>
          </div>
        ))}
        {!isExpanded && hasMore && (
          <div className="text-text-tertiary">+ {entries.length - 3} more... <span className="text-[9px]">(click to expand)</span></div>
        )}
        {isExpanded && hasMore && (
          <div className="text-text-tertiary text-[9px] mt-1">(click to collapse)</div>
        )}
      </div>
    </div>
  )
}
