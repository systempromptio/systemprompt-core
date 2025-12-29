import { Loader, AlertCircle, Check, Wrench, Lightbulb } from 'lucide-react'
import { cn } from '@/lib/utils/cn'
import type { ExecutionStep } from '@/types/execution'
import { getToolName, getToolArguments, getReasoning, getPlannedTools, getStepTitle, getStepSubtitle } from '@/types/execution'
import { formatDuration, getStatusColor } from './utils'
import { MorphingShape, useElapsedTime, ExpandableToolArgs } from './shared-components'

interface StreamingViewProps {
  steps: ExecutionStep[]
  variant?: 'standalone' | 'bubble'
  className?: string
}

export function StreamingView({ steps, className }: StreamingViewProps) {
  const currentStep = steps.find(s => s.status === 'in_progress') ?? steps[steps.length - 1]
  const completed = steps.filter(s => s.status === 'completed').length
  const hasFailed = steps.some(s => s.status === 'failed')

  const isStreaming = true
  const elapsed = useElapsedTime(currentStep?.startedAt, isStreaming)

  const activeStep = currentStep ?? steps[steps.length - 1]
  const toolName = activeStep ? getToolName(activeStep) : undefined
  const toolArgs = activeStep ? getToolArguments(activeStep) : undefined
  const reasoning = activeStep ? getReasoning(activeStep) : undefined
  const plannedTools = activeStep ? getPlannedTools(activeStep) : undefined
  const subtitle = activeStep ? getStepSubtitle(activeStep) : undefined

  const stepProgress = steps.length > 0 ? `${completed}/${steps.length}` : '0/0'
  const title = currentStep ? getStepTitle(currentStep) : 'PROCESSING REQUEST...'

  return (
    <div className={cn('space-y-3', className)}>
      <div className="flex items-start gap-3">
        <div className="relative flex-shrink-0">
          <div className={cn(
            'w-10 h-10 rounded-full flex items-center justify-center transition-colors duration-300',
            hasFailed ? 'bg-error/20' : 'bg-primary/20'
          )}>
            {hasFailed ? (
              <AlertCircle className="w-6 h-6 text-error" />
            ) : (
              <MorphingShape />
            )}
          </div>
        </div>

        <div className="flex-1 min-w-0">
          <h4 className="text-sm font-medium text-text-primary leading-tight uppercase transition-all duration-200">
            {title}
          </h4>
          <div className="flex items-center gap-1.5 text-xs text-text-tertiary mt-0.5">
            <span className="tabular-nums uppercase">STEP {stepProgress}</span>
            {elapsed && (
              <>
                <span className="text-text-muted">•</span>
                <span className="tabular-nums">{elapsed}</span>
              </>
            )}
          </div>
        </div>
      </div>

      {steps.length > 0 && (
        <div className="flex items-center px-1">
          {steps.map((step, i) => {
            const stepDuration = step.durationMs ?? (step.completedAt && step.startedAt
              ? new Date(step.completedAt).getTime() - new Date(step.startedAt).getTime()
              : null)

            return (
              <div key={step.stepId} className="flex items-center group relative">
                <div
                  className={cn(
                    'w-5 h-5 rounded-full flex items-center justify-center transition-all text-[10px]',
                    getStatusColor(step.status),
                    step.status === 'in_progress' && 'ring-2 ring-primary/40 ring-offset-1 scale-110'
                  )}
                >
                  {step.status === 'in_progress' ? (
                    <Loader className="w-2.5 h-2.5 animate-spin" />
                  ) : step.status === 'completed' ? (
                    <Check className="w-2.5 h-2.5" />
                  ) : step.status === 'failed' ? (
                    <AlertCircle className="w-2.5 h-2.5" />
                  ) : (
                    <span className="text-[8px] font-bold">{i + 1}</span>
                  )}
                </div>

                <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 hidden group-hover:block z-10">
                  <div className="bg-surface border border-border rounded-md shadow-lg p-2 whitespace-nowrap min-w-[120px]">
                    <div className="text-xs font-medium uppercase">{getStepTitle(step)}</div>
                    {stepDuration !== null && stepDuration > 0 && (
                      <div className="text-[10px] text-text-tertiary mt-0.5">
                        DURATION: {formatDuration(stepDuration)}
                      </div>
                    )}
                    {getStepSubtitle(step) && (
                      <div className="text-[10px] text-text-tertiary mt-0.5 uppercase">{getStepSubtitle(step)}</div>
                    )}
                  </div>
                </div>

                {i < steps.length - 1 && (
                  <div className={cn(
                    'w-3 h-0.5',
                    step.status === 'completed' ? 'bg-success' : 'bg-muted'
                  )} />
                )}
              </div>
            )
          })}

          <div className="flex items-center gap-1 ml-2">
            <div className="w-2 h-2 rounded-full bg-muted animate-pulse" />
            <div className="w-2 h-2 rounded-full bg-muted/60 animate-pulse" style={{ animationDelay: '150ms' }} />
            <div className="w-2 h-2 rounded-full bg-muted/30 animate-pulse" style={{ animationDelay: '300ms' }} />
          </div>
        </div>
      )}

      {(toolName || reasoning || plannedTools || subtitle || activeStep?.content.type === 'tool_execution') && (
        <div className="bg-muted/30 rounded-md p-2 border border-border/50">
          <div className="flex items-start gap-2">
            <div className="text-text-tertiary mt-0.5">
              {toolName || activeStep?.content.type === 'tool_execution' ? <Wrench className="w-3.5 h-3.5" /> : <Lightbulb className="w-3.5 h-3.5" />}
            </div>
            <div className="flex-1 min-w-0">
              {toolName && (
                <div className="flex items-center gap-2">
                  <code className="text-xs font-mono bg-primary/10 text-primary px-1.5 py-0.5 rounded uppercase">
                    {toolName.replace(/_/g, ' ')}
                  </code>
                </div>
              )}
              {toolArgs && Object.keys(toolArgs).length > 0 && (
                <div className="mt-1.5">
                  <ExpandableToolArgs args={toolArgs as Record<string, unknown>} />
                </div>
              )}
              {reasoning && (
                <p className="text-xs text-text-secondary mt-1 line-clamp-2 uppercase">{reasoning}</p>
              )}
              {plannedTools && plannedTools.length > 0 && (
                <div className="mt-2 space-y-1">
                  <div className="text-[10px] text-text-tertiary uppercase font-medium">PLANNED TOOLS ({plannedTools.length})</div>
                  <div className="flex flex-wrap gap-1">
                    {plannedTools.map((tool, i) => (
                      <code key={i} className="text-[10px] font-mono bg-secondary/10 text-secondary px-1.5 py-0.5 rounded uppercase">
                        {tool.tool_name.replace(/_/g, ' ')}
                      </code>
                    ))}
                  </div>
                </div>
              )}
              {activeStep?.content.type === 'tool_execution' && !toolArgs && (
                <p className="text-xs text-text-tertiary mt-1 uppercase">EXECUTING...</p>
              )}
              {activeStep?.content.type === 'tool_execution' && toolArgs && Object.keys(toolArgs).length === 0 && (
                <p className="text-xs text-text-tertiary mt-1 uppercase">NO PARAMETERS</p>
              )}
              {!reasoning && !toolName && !plannedTools && subtitle && (
                <p className="text-xs text-text-secondary mt-1 uppercase">{subtitle}</p>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
