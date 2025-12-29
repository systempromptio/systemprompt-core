import { useState } from 'react'
import { ChevronDown, ChevronUp } from 'lucide-react'
import { cn } from '@/lib/utils/cn'
import type { ExecutionStep } from '@/types/execution'
import { ModalStepList } from './ModalStepCard'

interface StaticViewProps {
  steps: ExecutionStep[]
  initialCollapsed?: boolean
  className?: string
}

export function StaticView({ steps, initialCollapsed = false, className }: StaticViewProps) {
  const [isCollapsed, setIsCollapsed] = useState(initialCollapsed)
  const completed = steps.filter(s => s.status === 'completed').length
  const hasFailed = steps.some(s => s.status === 'failed')

  return (
    <div className={cn('border-t border-border bg-muted/30', className)}>
      <button
        onClick={() => setIsCollapsed(!isCollapsed)}
        className="w-full flex items-center justify-between px-3 py-2.5 hover:bg-muted/50 transition-colors"
      >
        <div className="flex items-center gap-2">
          <h4 className="text-sm font-semibold text-text-primary">Execution Steps</h4>
          <span className={cn(
            'text-[11px] px-1.5 py-0.5 rounded-full tabular-nums',
            hasFailed ? 'bg-error/10 text-error' : 'bg-success/10 text-success'
          )}>
            {completed}/{steps.length}
          </span>
        </div>
        {isCollapsed ? <ChevronDown className="w-4 h-4 text-text-tertiary" /> : <ChevronUp className="w-4 h-4 text-text-tertiary" />}
      </button>
      {!isCollapsed && (
        <div className="px-3 pb-3">
          <ModalStepList steps={steps} />
        </div>
      )}
    </div>
  )
}
