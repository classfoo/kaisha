import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import type { DevTask, DevTaskStatus } from '../features/requirements/requirementsApi'

type RequirementDevelopmentSectionProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  t: (key: string) => string
}

function taskStatusLabel(t: (key: string) => string, status: DevTaskStatus) {
  return t(`ui.requirements.development.taskStatus.${status}`)
}

function taskStatusColor(status: DevTaskStatus): string {
  switch (status) {
    case 'branch_created':
      return '#9ba7be'
    case 'in_development':
      return '#9ed0ff'
    case 'dev_complete':
      return '#b8e6c8'
    case 'in_review':
      return '#e0c8a8'
    case 'review_complete':
      return '#c8a8e0'
    case 'merged':
      return '#4ade80'
  }
}

function taskStatusBg(status: DevTaskStatus): string {
  switch (status) {
    case 'branch_created':
      return '#1a2230'
    case 'in_development':
      return '#1a2d4a'
    case 'dev_complete':
      return '#1a3028'
    case 'in_review':
      return '#3a2a1a'
    case 'review_complete':
      return '#2a1a3a'
    case 'merged':
      return '#1a3028'
  }
}

const TASK_ACTIONS: Record<DevTaskStatus, { label: (key: string) => string; action: string }[]> = {
  branch_created: [{ label: () => `ui.requirements.development.actions.startDev`, action: 'start_development' }],
  in_development: [
    { label: () => `ui.requirements.development.actions.continueDev`, action: 'continue_development' },
    { label: () => `ui.requirements.development.actions.completeDev`, action: 'complete_development' },
  ],
  dev_complete: [
    { label: () => `ui.requirements.development.actions.reviewCode`, action: 'review_code' },
    { label: () => `ui.requirements.development.actions.startReview`, action: 'start_review' },
  ],
  in_review: [
    { label: () => `ui.requirements.development.actions.reviewCode`, action: 'review_code' },
    { label: () => `ui.requirements.development.actions.completeReview`, action: 'complete_review' },
  ],
  review_complete: [{ label: () => `ui.requirements.development.actions.merge`, action: 'merge' }],
  merged: [],
}

function DevTaskRow({
  task,
  requirements,
  t,
}: {
  task: DevTask
  requirements: ReturnType<typeof useRequirementsWorkspace>
  t: (key: string) => string
}) {
  const { detail, devActionKey, devTaskAction, updateDevTaskAction } = requirements
  const actions = TASK_ACTIONS[task.status]
  const isBusy = devActionKey === `${task.id}:*` || devActionKey?.startsWith(`${task.id}:`)

  const handleProgressChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (!detail) return
    const progress = Math.min(100, Math.max(0, Number(e.target.value)))
    void updateDevTaskAction(detail.id, task.id, { progress })
  }

  return (
    <div className="requirement-development__task">
      <div className="requirement-development__task-header">
        <span className="requirement-development__task-title">{task.title}</span>
        <span
          className="requirement-development__task-status"
          style={{ color: taskStatusColor(task.status), background: taskStatusBg(task.status) }}
        >
          {taskStatusLabel(t, task.status)}
        </span>
      </div>
      <div className="requirement-development__task-meta">
        <span className="requirement-development__task-branch">{task.branch}</span>
        {task.assignee && <span className="requirement-development__task-assignee">{task.assignee}</span>}
      </div>
      <div className="requirement-development__progress">
        <div className="requirement-development__progress-bar">
          <div
            className="requirement-development__progress-fill"
            style={{ width: `${task.progress}%` }}
          />
        </div>
        <span className="requirement-development__progress-label">{task.progress}%</span>
      </div>
      <input
        type="range"
        min={0}
        max={100}
        value={task.progress}
        onChange={handleProgressChange}
        className="requirement-development__progress-slider"
        disabled={task.status === 'merged'}
      />
      {actions.length > 0 && (
        <div className="requirement-development__task-actions">
          {actions.map((a) => (
            <button
              key={a.action}
              type="button"
              className="action-btn action-btn--compact"
              onClick={() => {
                if (!detail) return
                void devTaskAction(detail.id, task.id, a.action)
              }}
              disabled={isBusy}
            >
              {isBusy ? t('ui.requirements.development.processing') : t(a.label(''))}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

export function RequirementDevelopmentSection({ requirements, t }: RequirementDevelopmentSectionProps) {
  const { detail, development, devLoading, createDevTaskAction, startDevelopmentAction } = requirements
  const [createTaskOpen, setCreateTaskOpen] = React.useState(false)
  const [taskTitle, setTaskTitle] = React.useState('')
  const [taskAssignee, setTaskAssignee] = React.useState('')
  const [createError, setCreateError] = React.useState('')

  if (devLoading && !development) {
    return <p className="settings-subtext">{t('ui.requirements.development.loading')}</p>
  }

  if (!development) {
    return (
      <div className="requirement-development">
        <p className="settings-subtext">{t('ui.requirements.development.notStarted')}</p>
        {detail && (
          <button
            type="button"
            className="action-btn"
            onClick={() => void startDevelopmentAction(detail.id)}
          >
            {t('ui.requirements.development.start')}
          </button>
        )}
      </div>
    )
  }

  return (
    <div className="requirement-development">
      <div className="requirement-development__feature-branch">
        <span className="settings-subtext">{t('ui.requirements.development.featureBranch')}:</span>
        <code className="requirement-development__branch-name">{development.feature_branch}</code>
      </div>

      {createTaskOpen && (
        <div className="requirement-review-confirm" role="dialog" aria-modal="true">
          <p className="requirement-review-confirm__text">{t('ui.requirements.development.createTaskText')}</p>
          <input
            type="text"
            className="workspace-setup__input"
            placeholder={t('ui.requirements.development.taskTitle')}
            value={taskTitle}
            onChange={(e) => setTaskTitle(e.target.value)}
            autoFocus
          />
          <input
            type="text"
            className="workspace-setup__input"
            placeholder={t('ui.requirements.development.taskAssignee')}
            value={taskAssignee}
            onChange={(e) => setTaskAssignee(e.target.value)}
          />
          <div className="requirement-review-confirm__actions">
            <button
              type="button"
              className="action-btn"
              onClick={() => {
                setCreateTaskOpen(false)
                setCreateError('')
              }}
            >
              {t('ui.requirements.development.cancel')}
            </button>
            <button
              type="button"
              className="action-btn"
              onClick={() => {
                if (!detail || !taskTitle.trim()) return
                setCreateError('')
                void createDevTaskAction(detail.id, {
                  title: taskTitle.trim(),
                  assignee: taskAssignee.trim() || undefined,
                })
                  .then(() => setCreateTaskOpen(false))
                  .catch((e) => setCreateError(e instanceof Error ? e.message : String(e)))
              }}
            >
              {t('ui.requirements.development.confirm')}
            </button>
          </div>
          {createError ? <p className="workspace-setup__error">{createError}</p> : null}
        </div>
      )}

      <div className="requirement-development__task-list">
        {development.tasks.length > 0 ? (
          development.tasks.map((task) => (
            <DevTaskRow key={task.id} task={task} requirements={requirements} t={t} />
          ))
        ) : (
          <p className="settings-subtext">{t('ui.requirements.development.noTasks')}</p>
        )}
      </div>
    </div>
  )
}
