import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import type { TestTask, TestTaskStatus } from '../features/requirements/requirementsApi'

type RequirementTestingSectionProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  t: (key: string) => string
}

function testStatusColor(status: TestTaskStatus): string {
  switch (status) {
    case 'pending':
      return '#9ba7be'
    case 'running':
      return '#9ed0ff'
    case 'completed':
      return '#4ade80'
    case 'failed':
      return '#f08a8a'
  }
}

function testStatusBg(status: TestTaskStatus): string {
  switch (status) {
    case 'pending':
      return '#1a2230'
    case 'running':
      return '#1a2d4a'
    case 'completed':
      return '#1a3028'
    case 'failed':
      return '#3a1a1a'
  }
}

function TestTaskRow({
  task,
  requirements,
  t,
}: {
  task: TestTask
  requirements: ReturnType<typeof useRequirementsWorkspace>
  t: (key: string) => string
}) {
  const { detail, agentActionKey, testTaskAction } = requirements
  const isBusy = agentActionKey?.startsWith(`${task.id}:`)
  const canExecute = task.status === 'pending' || task.status === 'failed'

  return (
    <div className="requirement-development__task">
      <div className="requirement-development__task-header">
        <span className="requirement-development__task-title">{task.title}</span>
        <span
          className="requirement-development__task-status"
          style={{ color: testStatusColor(task.status), background: testStatusBg(task.status) }}
        >
          {t(`ui.requirements.testing.taskStatus.${task.status}`)}
        </span>
      </div>
      {task.assignee && (
        <div className="requirement-development__task-meta">
          <span className="requirement-development__task-assignee">{task.assignee}</span>
        </div>
      )}
      {canExecute && (
        <div className="requirement-development__task-actions">
          <button
            type="button"
            className="action-btn action-btn--compact"
            onClick={() => {
              if (!detail) return
              void testTaskAction(detail.id, task.id, 'execute')
            }}
            disabled={isBusy}
          >
            {isBusy ? t('ui.requirements.testing.processing') : t('ui.requirements.testing.execute')}
          </button>
        </div>
      )}
    </div>
  )
}

export function RequirementTestingSection({ requirements, t }: RequirementTestingSectionProps) {
  const { detail, testing, testingLoading, reloadTesting } = requirements

  React.useEffect(() => {
    if (detail) void reloadTesting(detail.id)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detail?.id])

  if (testingLoading && !testing) {
    return <p className="settings-subtext">{t('ui.requirements.testing.loading')}</p>
  }

  return (
    <div className="requirement-development">
      <div className="requirement-development__task-list">
        {testing && testing.tasks.length > 0 ? (
          testing.tasks.map((task) => (
            <TestTaskRow key={task.id} task={task} requirements={requirements} t={t} />
          ))
        ) : (
          <p className="settings-subtext">{t('ui.requirements.testing.noTasks')}</p>
        )}
      </div>
    </div>
  )
}
