import { Loader, AlertCircle, ChevronRight, Code, Wrench, Sparkles, Brain, Lightbulb, MapPin, Check } from 'lucide-react'
import { cn } from '@/lib/utils/cn'
import { useExpandableList } from '@/hooks/useExpandableList'
import type { ExecutionStep } from '@/types/execution'
import { getToolName, getToolArguments, getToolResult, getReasoning, getPlannedTools, getStepTitle, getStepSubtitle, getSkillId } from '@/types/execution'
import { formatDuration, getStepIcon, getStatusColor, getStatusBorderColor } from './utils'
import { DetailSection, ExpandableJson } from './shared-components'

interface ModalStepListProps {
  steps: ExecutionStep[]
}

export function ModalStepList({ steps }: ModalStepListProps) {
  const { isExpanded, toggle } = useExpandableList()
  const completed = steps.filter(s => s.status === 'completed').length
  const hasFailed = steps.some(s => s.status === 'failed')

  const totalDuration = steps.reduce((acc, step) => {
    const stepDuration = step.durationMs ?? (step.completedAt && step.startedAt
      ? new Date(step.completedAt).getTime() - new Date(step.startedAt).getTime()
      : 0)
    return acc + stepDuration
  }, 0)

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2 mb-3">
        <span className={cn(
          'text-[11px] px-1.5 py-0.5 rounded-full tabular-nums font-medium',
          hasFailed ? 'bg-error/10 text-error' : 'bg-muted text-text-secondary'
        )}>
          {completed}/{steps.length} COMPLETED
        </span>
        {totalDuration > 0 && (
          <span className="text-[11px] px-1.5 py-0.5 rounded-full tabular-nums font-medium bg-muted text-text-secondary">
            TOTAL: {formatDuration(totalDuration)}
          </span>
        )}
      </div>
      {steps.map((step, index) => (
        <ModalStepCard key={step.stepId} step={step} index={index} isExpanded={isExpanded(step.stepId)} onToggle={() => toggle(step.stepId)} />
      ))}
    </div>
  )
}

interface ModalStepCardProps {
  step: ExecutionStep
  index: number
  isExpanded: boolean
  onToggle: () => void
}

function ModalStepCard({ step, index, isExpanded, onToggle }: ModalStepCardProps) {
  const toolName = getToolName(step)
  const toolArgs = getToolArguments(step)
  const toolResult = getToolResult(step)
  const reasoning = getReasoning(step)
  const plannedTools = getPlannedTools(step)
  const skillId = getSkillId(step)
  const duration = step.durationMs ?? (step.completedAt && step.startedAt ? new Date(step.completedAt).getTime() - new Date(step.startedAt).getTime() : null)

  const hasDetail = toolName || toolArgs || toolResult || reasoning || plannedTools || step.errorMessage || skillId ||
    step.content.type === 'understanding' || step.content.type === 'planning' || step.content.type === 'completion'

  return (
    <div
      className={cn(
        'border rounded-lg overflow-hidden transition-all',
        getStatusBorderColor(step.status),
        step.status === 'failed' && 'bg-error/5',
        step.status === 'in_progress' && 'bg-primary/5',
        isExpanded && 'ring-1 ring-primary/20'
      )}
    >
      <button
        onClick={onToggle}
        className="w-full flex items-start gap-2.5 p-3 text-left hover:bg-muted/30 transition-colors"
      >
        <div className={cn('w-5 h-5 rounded-full flex items-center justify-center flex-shrink-0 mt-0.5', getStatusColor(step.status))}>
          {step.status === 'in_progress' ? (
            <Loader className="w-2.5 h-2.5 animate-spin" />
          ) : step.status === 'completed' ? (
            <Check className="w-2.5 h-2.5" />
          ) : step.status === 'failed' ? (
            <AlertCircle className="w-2.5 h-2.5" />
          ) : (
            <span className="text-[9px] font-bold">{index + 1}</span>
          )}
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5 mb-0.5">
            <span className="text-text-tertiary flex-shrink-0">{getStepIcon(step.content.type)}</span>
            <h4 className="text-sm font-medium text-text-primary truncate leading-tight uppercase">{getStepTitle(step)}</h4>
          </div>
          {getStepSubtitle(step) && (
            <p className="text-xs text-text-tertiary truncate leading-snug uppercase">{getStepSubtitle(step)}</p>
          )}
          {duration !== null && duration > 0 && (
            <p className="text-[10px] text-text-muted tabular-nums mt-1">{formatDuration(duration)}</p>
          )}
        </div>

        {hasDetail && (
          <ChevronRight className={cn('w-4 h-4 text-text-tertiary transition-transform flex-shrink-0 mt-0.5', isExpanded && 'rotate-90')} />
        )}
      </button>

      {isExpanded && (
        <div className="border-t border-border bg-surface p-3 space-y-3">
          <div className="flex gap-4 text-[10px] text-text-tertiary">
            <span>Started: {new Date(step.startedAt).toLocaleTimeString()}</span>
            {step.completedAt && <span>Completed: {new Date(step.completedAt).toLocaleTimeString()}</span>}
          </div>

          {step.content.type === 'understanding' && (
            <DetailSection icon={<Brain className="w-3.5 h-3.5" />} label="Status">
              <p className="text-xs text-text-secondary">REQUEST RECEIVED AND PARSED</p>
            </DetailSection>
          )}

          {step.content.type === 'planning' && !reasoning && (
            <DetailSection icon={<MapPin className="w-3.5 h-3.5" />} label="Status">
              <p className="text-xs text-text-secondary">DETERMINING EXECUTION STRATEGY</p>
            </DetailSection>
          )}

          {reasoning && (
            <DetailSection icon={<Lightbulb className="w-3.5 h-3.5" />} label="Reasoning">
              <p className="text-xs whitespace-pre-wrap">{reasoning}</p>
            </DetailSection>
          )}

          {plannedTools && plannedTools.length > 0 && (
            <DetailSection icon={<Wrench className="w-3.5 h-3.5" />} label={`Planned Tools (${plannedTools.length})`} collapsible defaultOpen>
              <div className="space-y-2">
                {plannedTools.map((tool, i) => (
                  <div key={i} className="border border-border/50 rounded p-2">
                    <code className="text-xs font-mono bg-secondary/10 text-secondary px-1.5 py-0.5 rounded uppercase">
                      {tool.tool_name.replace(/_/g, ' ')}
                    </code>
                    {tool.arguments && typeof tool.arguments === 'object' && Object.keys(tool.arguments as Record<string, unknown>).length > 0 ? (
                      <div className="mt-2">
                        <ExpandableJson data={tool.arguments as Record<string, unknown>} />
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>
            </DetailSection>
          )}

          {step.content.type === 'completion' && (
            <DetailSection icon={<Check className="w-3.5 h-3.5" />} label="Status">
              <p className="text-xs text-text-secondary">TASK EXECUTION COMPLETED SUCCESSFULLY</p>
            </DetailSection>
          )}

          {toolName && (
            <DetailSection icon={<Wrench className="w-3.5 h-3.5" />} label="Tool">
              <code className="text-xs font-mono bg-primary/10 text-primary px-1.5 py-0.5 rounded uppercase">{toolName.replace(/_/g, ' ')}</code>
            </DetailSection>
          )}

          {skillId && (
            <DetailSection icon={<Sparkles className="w-3.5 h-3.5" />} label="Skill ID">
              <code className="text-xs font-mono bg-secondary/10 text-secondary px-1.5 py-0.5 rounded">{skillId}</code>
            </DetailSection>
          )}

          {toolArgs && Object.keys(toolArgs).length > 0 && (
            <DetailSection icon={<Code className="w-3.5 h-3.5" />} label="Input" collapsible defaultOpen={false}>
              <ExpandableJson data={toolArgs} />
            </DetailSection>
          )}

          {toolResult && (
            <DetailSection icon={<Code className="w-3.5 h-3.5" />} label="Output" collapsible defaultOpen>
              <ExpandableJson data={toolResult} />
            </DetailSection>
          )}

          {step.errorMessage && (
            <div className="p-2 bg-error/10 border border-error/30 rounded">
              <div className="flex items-center gap-1.5 text-error text-xs font-medium mb-1">
                <AlertCircle className="w-3.5 h-3.5" />
                Error
              </div>
              <p className="text-xs text-error/90 whitespace-pre-wrap">{step.errorMessage}</p>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
