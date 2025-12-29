import type { Task } from '@/types/task'

export function parseSQLiteDateTime(dateStr: string | null): number {
  if (!dateStr) return 0

  try {
    const sqliteFormat = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/
    if (sqliteFormat.test(dateStr)) {
      return new Date(dateStr.replace(' ', 'T')).getTime()
    }
    return new Date(dateStr).getTime()
  } catch {
    return 0
  }
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`

  const totalSeconds = Math.floor(ms / 1000)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60

  if (hours > 0) {
    return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`
  }
  if (minutes > 0) {
    return seconds > 0 ? `${minutes}m ${seconds}s` : `${minutes}m`
  }
  return `${seconds}s`
}

export function calculateTaskDuration(task: Task): string {
  if (!task.status) return 'No status'

  const isCompleted = ['completed', 'failed', 'rejected'].includes(task.status.state)

  if (!isCompleted) {
    return 'In Progress'
  }

  const { execution_time_ms, started_at, completed_at } = task.metadata

  if (execution_time_ms !== undefined) {
    return formatDuration(execution_time_ms)
  }

  if (started_at && completed_at) {
    const startedMs = parseSQLiteDateTime(started_at)
    const completedMs = parseSQLiteDateTime(completed_at)
    if (startedMs > 0 && completedMs > 0) {
      return formatDuration(completedMs - startedMs)
    }
  }

  return 'No timing data'
}

export interface TaskStatusInfo {
  label: string
  className: string
  color: 'success' | 'danger' | 'warning' | 'info' | 'default'
  icon: string
}

const STATUS_INFO_MAP: Record<string, TaskStatusInfo> = {
  submitted: {
    label: 'Submitted',
    className: 'status-submitted',
    color: 'info',
    icon: 'send',
  },
  working: {
    label: 'Working',
    className: 'status-working',
    color: 'info',
    icon: 'hourglass',
  },
  'input-required': {
    label: 'Input Required',
    className: 'status-input-required',
    color: 'warning',
    icon: 'question',
  },
  completed: {
    label: 'Completed',
    className: 'status-completed',
    color: 'success',
    icon: 'checkmark',
  },
  failed: {
    label: 'Failed',
    className: 'status-failed',
    color: 'danger',
    icon: 'error',
  },
  rejected: {
    label: 'Rejected',
    className: 'status-rejected',
    color: 'danger',
    icon: 'block',
  },
  canceled: {
    label: 'Canceled',
    className: 'status-canceled',
    color: 'default',
    icon: 'close',
  },
  'auth-required': {
    label: 'Auth Required',
    className: 'status-auth-required',
    color: 'warning',
    icon: 'lock',
  },
}

export function getTaskStatusInfo(state: string): TaskStatusInfo {
  return STATUS_INFO_MAP[state] || {
    label: 'Unknown',
    className: 'status-unknown',
    color: 'default',
    icon: 'help',
  }
}

export function isTaskCompleted(state: string): boolean {
  return ['completed', 'failed', 'rejected', 'canceled'].includes(state)
}

export function isTaskRunning(state: string): boolean {
  return ['working', 'submitted', 'input-required'].includes(state)
}

export function formatTaskTimestamp(timestamp: string | null, includeTime = false): string {
  if (!timestamp) return ''

  try {
    const date = new Date(timestamp)
    return includeTime ? date.toLocaleString() : date.toLocaleDateString()
  } catch {
    return ''
  }
}

export function sortTasksByTimestamp(tasks: Task[]): Task[] {
  return [...tasks].sort((a, b) => {
    const timeA = a.status?.timestamp ? parseSQLiteDateTime(a.status.timestamp) : 0
    const timeB = b.status?.timestamp ? parseSQLiteDateTime(b.status.timestamp) : 0
    return timeB - timeA
  })
}

export function filterTasksByState(tasks: Task[], states: string | string[]): Task[] {
  const stateArray = Array.isArray(states) ? states : [states]
  return tasks.filter(task => stateArray.includes(task.status?.state || ''))
}

export function getTaskAgentName(task: Task): string {
  return task.metadata.agent_name || 'Unknown Agent'
}

export function getTaskMcpServer(task: Task): string {
  return task.metadata.mcp_server_name || 'Unknown Server'
}
