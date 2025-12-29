import React, { useEffect, useMemo, useState } from 'react'
import { MessageView } from './MessageView'
import { TaskArtifacts } from './TaskArtifacts'
import { TaskMetadata } from './metadata/TaskMetadata'
import { useUIStateStore } from '@/stores/ui-state.store'
import { useAgentStore } from '@/stores/agent.store'
import { useArtifactStore } from '@/stores/artifact.store'
import { Modal, ModalBody } from '@/components/ui'
import { AgentCard } from '@/components/agents/AgentCard'
import type { Task } from '@/types/task'
import { createContextId, createArtifactId } from '@/types/core/brand'

interface TaskViewProps {
  task: Task
  contextId: string
}

export const TaskView = React.memo(function TaskView({ task, contextId }: TaskViewProps) {
  const [showAgentModal, setShowAgentModal] = useState(false)

  const activeStreamingTaskId = useUIStateStore((s) => s.activeStreamingTaskId)
  const addSteps = useUIStateStore((s) => s.addSteps)
  const openArtifacts = useArtifactStore((s) => s.openArtifacts)

  const agents = useAgentStore((s) => s.agents)

  const isStreaming = activeStreamingTaskId === task.id

  useEffect(() => {
    const metadataSteps = task.metadata?.executionSteps
    if (metadataSteps?.length) {
      addSteps(metadataSteps, createContextId(contextId))
    }
  }, [task.id, task.metadata?.executionSteps, addSteps, contextId])

  const taskAgent = useMemo(
    () => agents.find((a) => a.name === task.metadata?.agent_name),
    [agents, task.metadata?.agent_name]
  )

  const artifactCount = task.artifacts?.length || 0

  const metadataElement = !isStreaming ? (
    <TaskMetadata
      task={task}
      contextId={contextId}
      artifactCount={artifactCount}
      onArtifactClick={artifactCount > 0 ? () => openArtifacts(task.artifacts?.map(a => createArtifactId(a.artifactId)) || []) : undefined}
    />
  ) : null

  return (
    <div className="task-view space-y-0" data-task-id={task.id}>
      {task.history?.map((message) => (
        <MessageView
          key={message.messageId}
          message={message}
          agent={taskAgent}
          isStreaming={isStreaming && message.role === 'agent'}
          onAgentClick={() => taskAgent && setShowAgentModal(true)}
        />
      ))}

      {/* Metadata shows below messages when task is complete (indented to match agent avatar) */}
      {!isStreaming && metadataElement && (
        <div className="flex gap-3">
          {/* Spacer matching avatar width (sm = 32px / 2rem) */}
          <div className="w-8 flex-shrink-0" />
          <div className="flex-1 min-w-0">
            {metadataElement}
          </div>
        </div>
      )}

      {task.artifacts && task.artifacts.length > 0 && (
        <TaskArtifacts
          task={task}
          contextId={contextId}
        />
      )}

      {taskAgent && (
        <Modal
          isOpen={showAgentModal}
          onClose={() => setShowAgentModal(false)}
          title={taskAgent.name}
          size="md"
        >
          <ModalBody className="max-h-[70vh] overflow-y-auto">
            <AgentCard agent={taskAgent} />
          </ModalBody>
        </Modal>
      )}
    </div>
  )
})
