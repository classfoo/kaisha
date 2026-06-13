import React from 'react'
import type { AgentTaskDetail, AgentTaskKind, AgentTaskRecord, AgentTaskStatus } from '../features/employee-tasks/employeeTasksApi'
import { EmployeeTaskDetailDialog } from './EmployeeTaskDetailDialog'

type EmployeeTaskListProps = {
  tasks: AgentTaskRecord[]
  loading: boolean
  error: string | null
  selectedEmployeeId: string | null
  selectedEmployeeName: string | null
  locale: string
  exploring: boolean
  onExplore: () => void
  refreshing: boolean
  onRefresh: () => void
  rerunningTaskId: string | null
  onRerunTask: (taskId: string) => void
  stoppingTaskId: string | null
  onStopTask: (taskId: string) => void
  onFetchTaskDetail: (taskId: string) => Promise<AgentTaskDetail>
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

function canRerunTask(task: AgentTaskRecord): boolean {
  return (
    task.status === 'pending' ||
    task.status === 'running' ||
    task.status === 'completed' ||
    task.status === 'failed' ||
    task.status === 'cancelled' ||
    task.status === 'queued_rerun'
  )
}

function canStopTask(task: AgentTaskRecord): boolean {
  return task.status === 'pending' || task.status === 'running'
}

function isTaskOutputPending(status: AgentTaskStatus): boolean {
  return status === 'pending' || status === 'running' || status === 'queued_rerun'
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
  refreshing,
  onRefresh,
  rerunningTaskId,
  onRerunTask,
  stoppingTaskId,
  onStopTask,
  onFetchTaskDetail,
  t,
}: EmployeeTaskListProps) {
  const [openMenuId, setOpenMenuId] = React.useState<string | null>(null)
  const [detailTaskId, setDetailTaskId] = React.useState<string | null>(null)
  const [detail, setDetail] = React.useState<AgentTaskDetail | null>(null)
  const [detailLoading, setDetailLoading] = React.useState(false)
  const [detailError, setDetailError] = React.useState<string | null>(null)
  const exploreDisabled = false

  const closeDetail = React.useCallback(() => {
    setDetailTaskId(null)
    setDetail(null)
    setDetailError(null)
    setDetailLoading(false)
  }, [])

  const loadDetail = React.useCallback(
    async (taskId: string, options?: { silent?: boolean }) => {
      if (!options?.silent) {
        setDetailLoading(true)
        setDetailError(null)
      }
      try {
        const next = await onFetchTaskDetail(taskId)
        setDetail(next)
      } catch (err) {
        setDetailError(err instanceof Error && err.message ? err.message : t('ui.employeeTasks.detail.loadError'))
      } finally {
        if (!options?.silent) setDetailLoading(false)
      }
    },
    [onFetchTaskDetail, t],
  )

  const openDetail = React.useCallback(
    (taskId: string) => {
      setOpenMenuId(null)
      setDetailTaskId(taskId)
      setDetail(null)
      void loadDetail(taskId)
    },
    [loadDetail],
  )

  React.useEffect(() => {
    if (!detailTaskId || !detail?.task) return
    if (!isTaskOutputPending(detail.task.status)) return
    const timer = window.setInterval(() => {
      void loadDetail(detailTaskId, { silent: true })
    }, 3000)
    return () => window.clearInterval(timer)
  }, [detailTaskId, detail?.task?.status, loadDetail])

  const toolbar = (
    <div className="employee-task-list__toolbar">
      <h4 className="employee-task-list__title">{t('ui.employeeTasks.title')}</h4>
      {selectedEmployeeId ? (
        <>
          <button
            type="button"
            className="employee-task-list__tool-btn"
            title={t('ui.employeeTasks.refresh')}
            aria-label={t('ui.employeeTasks.refresh')}
            onClick={onRefresh}
            disabled={refreshing}
          >
            <i className={`iconfont ${refreshing ? 'icon-loading' : 'icon-filmeicon'}`} aria-hidden="true" />
            <span>{refreshing ? t('ui.employeeTasks.refreshing') : t('ui.employeeTasks.refresh')}</span>
          </button>
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
        </>
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
            const isStopping = stoppingTaskId === task.id
            const menuBusy = isRerunning || isStopping
            const rerunEnabled = canRerunTask(task) && !menuBusy
            const stopEnabled = canStopTask(task) && !menuBusy
            return (
              <div
                key={task.id}
                className={`employee-task-item employee-task-item--${task.status} ${menuOpen ? 'employee-task-item--menu-open' : ''}`}
                role="listitem"
              >
                <div
                  className="employee-task-item__body"
                  role="button"
                  tabIndex={0}
                  onClick={() => openDetail(task.id)}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                      event.preventDefault()
                      openDetail(task.id)
                    }
                  }}
                  aria-label={t('ui.employeeTasks.detail.title')}
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
                    disabled={menuBusy}
                  >
                    <i className="iconfont icon-filmetomore" aria-hidden="true" />
                  </button>
                  {menuOpen ? (
                    <div className="employee-item__menu-dropdown">
                      {canStopTask(task) ? (
                        <button
                          type="button"
                          className="employee-item__menu-dropdown-item employee-item__menu-dropdown-item--fire"
                          onClick={(e) => {
                            e.stopPropagation()
                            setOpenMenuId(null)
                            if (stopEnabled) onStopTask(task.id)
                          }}
                          disabled={!stopEnabled}
                        >
                          {isStopping ? t('ui.employeeTasks.stopping') : t('ui.employeeTasks.stop')}
                        </button>
                      ) : null}
                      {canRerunTask(task) ? (
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
                      ) : null}
                    </div>
                  ) : null}
                </div>
              </div>
            )
          })
        )}
      </div>
      <EmployeeTaskDetailDialog
        open={detailTaskId != null}
        loading={detailLoading}
        error={detailError}
        detail={detail}
        locale={locale}
        t={t}
        onClose={closeDetail}
      />
    </div>
  )
}
