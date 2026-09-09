export type TaskProgressStage = 'uploading' | 'parsing' | 'resolving_metadata' | 'analyzing_taste' | 'generating_recommendation' | 'completed' | 'failed'

export interface TaskProgressState {
  stage: TaskProgressStage
  detail?: string
  error?: string
}

const stages: { id: Exclude<TaskProgressStage, 'failed'>; label: string }[] = [
  { id: 'uploading', label: 'Uploading...' },
  { id: 'parsing', label: 'Parsing tracks...' },
  { id: 'resolving_metadata', label: 'Resolving metadata...' },
  { id: 'analyzing_taste', label: 'Analyzing taste...' },
  { id: 'generating_recommendation', label: 'Generating recommendation...' },
  { id: 'completed', label: 'Completed' },
]

export function TaskProgress({ state }: { state: TaskProgressState }) {
  const activeIndex = state.stage === 'failed' ? -1 : stages.findIndex((item) => item.id === state.stage)
  return <section className={`task-progress ${state.stage}`} role="status" aria-live="polite">
    <div className="task-progress-head">
      <strong>{state.stage === 'failed' ? 'Failed' : stages[activeIndex]?.label}</strong>
      {state.detail && <span>{state.detail}</span>}
    </div>
    <ol>
      {stages.map((item, index) => {
        const status = state.stage === 'failed' ? 'pending' : index < activeIndex ? 'done' : index === activeIndex ? 'active' : 'pending'
        return <li className={status} key={item.id}><i>{status === 'done' ? '✓' : status === 'active' ? '→' : '○'}</i><span>{item.label}</span></li>
      })}
    </ol>
    {state.stage === 'failed' && <p>{state.error || '任务失败，请重试。'}</p>}
  </section>
}
