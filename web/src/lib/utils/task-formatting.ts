import type { Task } from '@/types/task'

export function getAgentName(task: Task): string {
  return task.metadata.agent_name || ''
}

export function getMcpServerName(task: Task): string {
  return task.metadata.mcp_server_name || '-'
}

export function getConversationName(contextId: string, conversationMap: Map<string, { name: string }>): string {
  const conversation = conversationMap.get(contextId)

  if (!conversation) {
    return `New (${contextId.substring(0, 8)})`
  }

  if (!conversation.name) {
    return `Unnamed (${contextId.substring(0, 8)})`
  }

  return conversation.name
}

export function formatTaskStatus(state: string): {
  className: string
  text: string
} {
  const baseClasses = 'px-xs py-xs rounded text-xs font-medium whitespace-nowrap border'

  switch (state) {
    case 'completed':
      return {
        className: `${baseClasses} bg-success/20 text-success border-success/30`,
        text: state,
      }
    case 'failed':
    case 'rejected':
      return {
        className: `${baseClasses} bg-error/20 text-error border-error/30`,
        text: state,
      }
    case 'working':
      return {
        className: `${baseClasses} bg-warning/20 text-warning border-warning/30`,
        text: state,
      }
    default:
      return {
        className: `${baseClasses} bg-surface-variant text-text-secondary border-primary/10`,
        text: state,
      }
  }
}
