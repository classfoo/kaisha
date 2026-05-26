import React from 'react'
import { createPortal } from 'react-dom'
import type { AgentTaskDetail, AgentTaskKind, AgentTaskStatus } from '../features/employee-tasks/employeeTasksApi'

type EmployeeTaskDetailDialogProps = {
  open: boolean
  loading: boolean
  error: string | null
  detail: AgentTaskDetail | null
  locale: string
  t: (key: string) => string
  onClose: () => void
}

function taskKindKey(kind: AgentTaskKind): string {
  return `ui.employeeTasks.kind.${kind}`
}

function taskStatusKey(status: AgentTaskStatus): string {
  return `ui.employeeTasks.status.${status}`
}

function intlLocale(locale: string): string {
  if (locale === 'zh') return 'zh-CN'
  if (locale === 'ja') return 'ja-JP'
  return 'en-US'
}

function formatTime(ms: number | null | undefined, locale: string): string {
  if (ms == null) return '—'
  try {
    return new Intl.DateTimeFormat(intlLocale(locale), {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }).format(new Date(ms))
  } catch {
    return new Date(ms).toLocaleString()
  }
}

function DetailRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="task-detail-dialog__row">
      <div className="task-detail-dialog__label">{label}</div>
      <div className="task-detail-dialog__value">{value}</div>
    </div>
  )
}

export function EmployeeTaskDetailDialog({
  open,
  loading,
  error,
  detail,
  locale,
  t,
  onClose,
}: EmployeeTaskDetailDialogProps) {
  React.useEffect(() => {
    if (!open) return
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handleKey)
    return () => {
      document.body.style.overflow = previousOverflow
      document.removeEventListener('keydown', handleKey)
    }
  }, [open, onClose])

  if (!open) return null

  const task = detail?.task
  const execution = detail?.execution
  const outputText = detail?.output?.trim() ?? ''
  const outputPending =
    task?.status === 'pending' || task?.status === 'running' || task?.status === 'queued_rerun'

  return createPortal(
    <div className="task-detail-dialog" role="dialog" aria-modal="true" aria-label={t('ui.employeeTasks.detail.title')}>
      <div className="task-detail-dialog__overlay" onClick={onClose} />
      <div className="task-detail-dialog__content">
        <div className="task-detail-dialog__header">
          <h3 className="task-detail-dialog__title">{t('ui.employeeTasks.detail.title')}</h3>
          <button type="button" className="task-detail-dialog__close" onClick={onClose} aria-label={t('ui.employeeTasks.detail.close')}>
            ×
          </button>
        </div>

        {loading ? (
          <div className="task-detail-dialog__state">{t('ui.employeeTasks.detail.loading')}</div>
        ) : error ? (
          <div className="task-detail-dialog__state task-detail-dialog__state--error">{error}</div>
        ) : task && execution ? (
          <div className="task-detail-dialog__body">
            <section className="task-detail-dialog__section">
              <h4 className="task-detail-dialog__section-title">{t('ui.employeeTasks.detail.sectionInfo')}</h4>
              <dl className="task-detail-dialog__grid">
                <DetailRow label={t('ui.employeeTasks.detail.fields.id')} value={task.id} />
                <DetailRow label={t('ui.employeeTasks.detail.fields.kind')} value={t(taskKindKey(task.kind))} />
                <DetailRow label={t('ui.employeeTasks.detail.fields.status')} value={t(taskStatusKey(task.status))} />
                <DetailRow label={t('ui.employeeTasks.detail.fields.content')} value={task.content} />
                <DetailRow label={t('ui.employeeTasks.detail.fields.workdir')} value={task.workdir} />
                <DetailRow label={t('ui.employeeTasks.detail.fields.executor')} value={task.executor_id ?? '—'} />
                <DetailRow label={t('ui.employeeTasks.detail.fields.created')} value={formatTime(task.created_at_ms, locale)} />
                <DetailRow label={t('ui.employeeTasks.detail.fields.started')} value={formatTime(task.started_at_ms, locale)} />
                <DetailRow label={t('ui.employeeTasks.detail.fields.ended')} value={formatTime(task.ended_at_ms, locale)} />
                {task.parent_task_id ? (
                  <DetailRow label={t('ui.employeeTasks.detail.fields.parentTask')} value={task.parent_task_id} />
                ) : null}
                {task.context && Object.keys(task.context).length > 0 ? (
                  <DetailRow
                    label={t('ui.employeeTasks.detail.fields.context')}
                    value={<pre className="task-detail-dialog__code">{JSON.stringify(task.context, null, 2)}</pre>}
                  />
                ) : null}
              </dl>
            </section>

            <section className="task-detail-dialog__section">
              <h4 className="task-detail-dialog__section-title">{t('ui.employeeTasks.detail.sectionExecution')}</h4>
              <dl className="task-detail-dialog__grid">
                <DetailRow
                  label={t('ui.employeeTasks.detail.fields.tool')}
                  value={execution.tool_name ?? execution.tool_kind ?? '—'}
                />
                <DetailRow label={t('ui.employeeTasks.detail.fields.model')} value={execution.model ?? '—'} />
                <DetailRow
                  label={t('ui.employeeTasks.detail.fields.duration')}
                  value={
                    execution.duration_ms != null
                      ? t('ui.employeeTasks.detail.durationMs').replace('{ms}', String(execution.duration_ms))
                      : '—'
                  }
                />
                <DetailRow
                  label={t('ui.employeeTasks.detail.fields.tokens')}
                  value={t('ui.employeeTasks.detail.tokensSummary')
                    .replace('{prompt}', String(execution.prompt_tokens))
                    .replace('{completion}', String(execution.completion_tokens))
                    .replace('{total}', String(execution.total_tokens))}
                />
                <DetailRow
                  label={t('ui.employeeTasks.detail.fields.exitCode')}
                  value={execution.exit_code != null ? String(execution.exit_code) : '—'}
                />
                {execution.error ? (
                  <DetailRow label={t('ui.employeeTasks.detail.fields.error')} value={execution.error} />
                ) : null}
              </dl>
            </section>

            <section className="task-detail-dialog__section">
              <h4 className="task-detail-dialog__section-title">{t('ui.employeeTasks.detail.sectionOutput')}</h4>
              {outputText ? (
                <pre className="task-detail-dialog__output">{outputText}</pre>
              ) : (
                <div className="task-detail-dialog__state">
                  {outputPending ? t('ui.employeeTasks.detail.outputPending') : t('ui.employeeTasks.detail.outputEmpty')}
                </div>
              )}
            </section>
          </div>
        ) : null}

        <div className="task-detail-dialog__actions">
          <button type="button" className="task-detail-dialog__btn" onClick={onClose}>
            {t('ui.employeeTasks.detail.close')}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  )
}
