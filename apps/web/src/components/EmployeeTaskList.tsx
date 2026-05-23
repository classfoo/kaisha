import React from 'react'
import type { AgentTaskKind, AgentTaskRecord, AgentTaskStatus } from '../features/employee-tasks/employeeTasksApi'

type EmployeeTaskListProps = {
  tasks: AgentTaskRecord[]
  loading: boolean
  error: string | null
  selectedEmployeeId: string | null
  selectedEmployeeName: string | null
  locale: string
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

export function EmployeeTaskList({
  tasks,
  loading,
  error,
  selectedEmployeeId,
  selectedEmployeeName,
  locale,
  t,
}: EmployeeTaskListProps) {
  if (!selectedEmployeeId) {
    return (
      <div className="employee-task-list">
        <div className="employee-list__empty">{t('ui.employeeTasks.noSelection')}</div>
      </div>
    )
  }

  return (
    <div
      className="employee-task-list"
      role="list"
      aria-label={t('ui.employeeTasks.listTitle')}
    >
      {loading ? (
        <div className="employee-list__empty">{t('ui.employeeTasks.loading')}</div>
      ) : error ? (
        <div className="side-panel__error">{error}</div>
      ) : tasks.length === 0 ? (
        <div className="employee-list__empty">
          {t('ui.employeeTasks.empty').replace('{name}', selectedEmployeeName ?? '')}
        </div>
      ) : (
        tasks.map((task) => {
          const kindLabel = t(taskKindKey(task.kind))
          const statusLabel = t(taskStatusKey(task.status))
          return (
            <div
              key={task.id}
              className={`employee-task-item employee-task-item--${task.status}`}
              role="listitem"
            >
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
          )
        })
      )}
    </div>
  )
}
