import type { ReactElement } from 'react'
import { Brain, MapPin, Sparkles, Wrench, Check } from 'lucide-react'
import type { StepStatus, StepType } from '@/types/execution'

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  const seconds = ms / 1000
  if (seconds < 60) return `${seconds.toFixed(1)}s`
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}m ${secs}s`
}

export function formatElapsedTime(startedAt: string): string {
  const elapsed = (Date.now() - new Date(startedAt).getTime()) / 1000
  if (elapsed < 1) return '0s'
  if (elapsed < 60) return `${Math.floor(elapsed)}s`
  return `${Math.floor(elapsed / 60)}m ${Math.floor(elapsed % 60)}s`
}

export function getStepIcon(stepType: StepType): ReactElement {
  const iconClass = "w-3 h-3"
  switch (stepType) {
    case 'understanding': return <Brain className={iconClass} />
    case 'planning': return <MapPin className={iconClass} />
    case 'skill_usage': return <Sparkles className={iconClass} />
    case 'tool_execution': return <Wrench className={iconClass} />
    case 'completion': return <Check className={iconClass} />
  }
}

export function getStatusColor(status: StepStatus): string {
  switch (status) {
    case 'completed': return 'bg-emerald-500 text-white'
    case 'in_progress': return 'bg-blue-500 text-white'
    case 'failed': return 'bg-red-500 text-white'
    default: return 'bg-gray-400 text-white'
  }
}

export function getStatusBorderColor(status: StepStatus): string {
  switch (status) {
    case 'completed': return 'border-emerald-500'
    case 'in_progress': return 'border-blue-500'
    case 'failed': return 'border-red-500'
    default: return 'border-gray-400'
  }
}
