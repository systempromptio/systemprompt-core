import type { ReactElement } from 'react'
import type { ExecutionStep } from '@/types/execution'
import { StreamingView } from './StreamingView'
import { StaticView } from './StaticView'
import { ModalStepList } from './ModalStepCard'

type StreamingProps = {
  mode: 'streaming'
  steps: ExecutionStep[]
  variant?: 'standalone' | 'bubble'
  className?: string
}

type StaticProps = {
  mode: 'static'
  steps: ExecutionStep[]
  initialCollapsed?: boolean
  className?: string
}

type ModalProps = {
  mode: 'modal'
  steps: ExecutionStep[]
  className?: string
}

export type ExecutionTimelineProps = StreamingProps | StaticProps | ModalProps

function ModalView({ steps, className }: { steps: ExecutionStep[], className?: string }) {
  return (
    <div className={className}>
      <ModalStepList steps={steps} />
    </div>
  )
}

export function ExecutionTimeline(props: ExecutionTimelineProps): ReactElement | null {
  const { steps, className } = props

  if (steps.length === 0 && props.mode !== 'streaming') {
    return null
  }

  switch (props.mode) {
    case 'streaming':
      return <StreamingView steps={steps} variant={props.variant} className={className} />
    case 'static':
      return <StaticView steps={steps} initialCollapsed={props.initialCollapsed} className={className} />
    case 'modal':
      return <ModalView steps={steps} className={className} />
  }
}

export { StreamingView } from './StreamingView'
export { StaticView } from './StaticView'
export { ModalStepList } from './ModalStepCard'
