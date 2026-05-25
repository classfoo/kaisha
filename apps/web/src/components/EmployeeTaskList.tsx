import React from 'react'
import type { AgentTaskKind, AgentTaskRecord, AgentTaskStatus } from '../features/employee-tasks/employeeTasksApi'

type EmployeeTaskListProps = {
  tasks: AgentTaskRecord[]
  loading: boolean
  error: string | null
  selectedEmployeeId: string | null
  selectedEmployeeName: string | null
  locale: string
  exploring: boolean
  onExplore: () => void
  rerunningTaskId: string | null
  onRerunTask: (taskId: string) => void
  t: (key: string) => string
}

function taskKindKey(kind: AgentTaskKind): string {
  return `ui.employeeTasks.kind.${kind}`
}

function taskStatusKey(status: AgentTaskStatus): string {
  return `ui.employeeTasks.status.${status}`
}

function truncateContent(text: string, max = 72): string {
  const trimmed = text.trim()
  if (trimmed.length <= max) return trimmed
  return `${trimmed.slice(0, max)}…`
}

function intlLocale(locale: string): string {
  if (locale === 'zh') return 'zh-CN'
  if (locale === 'ja') return 'ja-JP'
  return 'en-US'
}

function formatTaskTime(ms: number, locale: string): string {
  try {
    return new Intl.DateTimeFormat(intlLocale(locale), {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(ms))
  } catch {
    return new Date(ms).toLocaleString()
  }
}

function isEmployeeBusy(tasks: AgentTaskRecord[]): boolean {
  return tasks.some((task) => task.status === 'pending' || task.status === 'running')
}

function canRerunTask(task: AgentTaskRecord): boolean {
  return task.status === 'completed' || task.status === 'failed' || task.status === 'cancelled'
}

export function EmployeeTaskList({
  tasks,
  loading,
  error,
  selectedEmployeeId,
  selectedEmployeeName,
  locale,
  exploring,
  onExplore,
  rerunningTaskId,
  onRerunTask,
  t,
}: EmployeeTaskListProps) {
  const [openMenuId, setOpenMenuId] = React.useState<string | null>(null)
  const exploreDisabled = exploring || loading || isEmployeeBusy(tasks)

  const toolbar = (
    <div className="employee-task-list__toolbar">
      <h4 className="employee-task-list__title">{t('ui.employeeTasks.title')}</h4>
      {selectedEmployeeId ? (
        <button
          type="button"
          className="employee-task-list__tool-btn"
          title={t('ui.employeeTasks.explore')}
          aria-label={t('ui.employeeTasks.explore')}
          onClick={onExplore}
          disabled={exploreDisabled}
        >
          <i className="iconfont icon-filmetotaosuo" aria-hidden="true" />
          <span>{exploring ? t('ui.employeeTasks.exploring') : t('ui.employeeTasks.explore')}</span>
        </button>
      ) : null}
    </div>
  )

  if (!selectedEmployeeId) {
    return (
      <div className="employee-task-list-wrap">
        {toolbar}
        <div className="employee-list__empty">{t('ui.employeeTasks.noSelection')}</div>
      </div>
    )
  }

  return (
    <div className="employee-task-list-wrap">
      {toolbar}
      {error ? <div className="side-panel__error">{error}</div> : null}
      <div className="employee-task-list" role="list" aria-label={t('ui.employeeTasks.listTitle')}>
        {loading ? (
          <div className="employee-list__empty">{t('ui.employeeTasks.loading')}</div>
        ) : tasks.length === 0 ? (
          <div className="employee-list__empty">
            {t('ui.employeeTasks.empty').replace('{name}', selectedEmployeeName ?? '')}
          </div>
        ) : (
          tasks.map((task) => {
            const kindLabel = t(taskKindKey(task.kind))
            const statusLabel = t(taskStatusKey(task.status))
            const menuOpen = openMenuId === task.id
            const isRerunning = rerunningTaskId === task.id
            const rerunEnabled = canRerunTask(task) && !isRerunning
            return (
              <div
                key={task.id}
                className={`employee-task-item employee-task-item--${task.status} ${menuOpen ? 'employee-task-item--menu-open' : ''}`}
                role="listitem"
              >
                <div className="employee-task-item__body">
                  <div className="employee-task-item__header">
                    <span className="employee-task-item__kind">{kindLabel}</span>
                    <span className={`employee-task-item__status employee-task-item__status--${task.status}`}>
                      {statusLabel}
                    </span>
                  </div>
                  <div className="employee-task-item__content">{truncateContent(task.content)}</div>
                  <div className="employee-task-item__meta">
                    {formatTaskTime(task.created_at_ms, locale)}
                  </div>
                </div>
                <div className={`employee-item__menu ${menuOpen ? 'employee-item__menu--open' : ''}`}>
                  <button
                    type="button"
                    className="employee-item__menu-btn"
                    title={t('ui.employeeTasks.menu')}
                    aria-label={t('ui.employeeTasks.menu')}
                    onClick={(e) => {
                      e.stopPropagation()
                      setOpenMenuId(menuOpen ? null : task.id)
                    }}
                    disabled={isRerunning}
                  >
                    <i className="iconfont icon-filmetomore" aria-hidden="true" />
                  </button>
                  {menuOpen ? (
                    <div className="employee-item__menu-dropdown">
                      <button
                        type="button"
                        className="employee-item__menu-dropdown-item"
                        onClick={(e) => {
                          e.stopPropagation()
                          setOpenMenuId(null)
                          if (rerunEnabled) onRerunTask(task.id)
                        }}
                        disabled={!rerunEnabled}
                      >
                        {isRerunning ? t('ui.employeeTasks.rerunning') : t('ui.employeeTasks.rerun')}
                      </button>
                    </div>
                  ) : null}
                </div>
              </div>
            )
          })
        )}
      </div>
    </div>
  )
}
