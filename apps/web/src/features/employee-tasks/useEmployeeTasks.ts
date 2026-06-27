import * as React from 'react'
import { AgentTaskRecord, createEmployeeTasksApi } from './employeeTasksApi'
import { isTransientTaskListError } from './isTransientTaskListError'

export const EMPLOYEE_TASKS_PAGE_SIZE = 10
const TASK_POLL_INTERVAL_MS = 2500
const RETRY_ATTEMPTS = 3
const RETRY_DELAY_MS = 500

export function hasActiveEmployeeTasks(tasks: AgentTaskRecord[]): boolean {
  return tasks.some(
    (task) => task.status === 'pending' || task.status === 'running' || task.status === 'queued_rerun',
  )
}

async function fetchWithRetry<T>(fn: () => Promise<T>, attempts: number, delayMs: number): Promise<T> {
  let lastError: Error | undefined
  for (let i = 0; i < attempts; i++) {
    try {
      return await fn()
    } catch (e) {
      lastError = e instanceof Error ? e : new Error(String(e))
      if (i < attempts - 1) {
        await new Promise((resolve) => setTimeout(resolve, delayMs * (i + 1)))
      }
    }
  }
  throw lastError
}

export function useEmployeeTasks(
  apiBase: string,
  locale: string,
  workspaceConfigured: boolean,
  selectedEmployeeId: string | null,
  refreshTick: number,
  pollWhileBusy = false,
) {
  const api = React.useMemo(() => createEmployeeTasksApi(apiBase, locale), [apiBase, locale])
  const [tasks, setTasks] = React.useState<AgentTaskRecord[]>([])
  const [total, setTotal] = React.useState(0)
  const [activeCount, setActiveCount] = React.useState(0)
  const [stoppableCount, setStoppableCount] = React.useState(0)
  const [page, setPage] = React.useState(1)
  const [loading, setLoading] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const tasksRef = React.useRef<AgentTaskRecord[]>([])
  tasksRef.current = tasks

  const pollWhileBusyRef = React.useRef(pollWhileBusy)
  pollWhileBusyRef.current = pollWhileBusy

  const selectedEmployeeIdRef = React.useRef(selectedEmployeeId)
  selectedEmployeeIdRef.current = selectedEmployeeId

  const pageRef = React.useRef(page)
  pageRef.current = page

  React.useEffect(() => {
    setPage(1)
  }, [selectedEmployeeId])

  React.useEffect(() => {
    const totalPages = Math.max(1, Math.ceil(total / EMPLOYEE_TASKS_PAGE_SIZE))
    if (page > totalPages) {
      setPage(totalPages)
    }
  }, [total, page])

  const refresh = React.useCallback(
    async (options?: { silent?: boolean; page?: number }) => {
      const silent = options?.silent ?? false
      const employeeId = selectedEmployeeIdRef.current
      const currentPage = options?.page ?? pageRef.current
      if (!workspaceConfigured || !employeeId) {
        setTasks([])
        setTotal(0)
        setActiveCount(0)
        setStoppableCount(0)
        setError(null)
        if (!silent) setLoading(false)
        return
      }
      if (!silent) {
        setLoading(true)
        setError(null)
      }
      try {
        const response = await fetchWithRetry(
          () =>
            api.listByExecutor(employeeId, {
              limit: EMPLOYEE_TASKS_PAGE_SIZE,
              offset: (currentPage - 1) * EMPLOYEE_TASKS_PAGE_SIZE,
            }),
          silent ? RETRY_ATTEMPTS : 1,
          RETRY_DELAY_MS,
        )
        if (selectedEmployeeIdRef.current !== employeeId) return
        setTasks(response.items)
        setTotal(response.total)
        setActiveCount(response.active_count)
        setStoppableCount(response.stoppable_count)
        setError(null)
      } catch (e) {
        if (selectedEmployeeIdRef.current !== employeeId) return
        const isTransient = e instanceof Error && isTransientTaskListError(e)
        const keepCachedTasks =
          tasksRef.current.length > 0 &&
          (pollWhileBusyRef.current || hasActiveEmployeeTasks(tasksRef.current))
        if (silent && isTransient) {
          return
        }
        if (!silent) {
          if (keepCachedTasks && isTransient) {
            return
          }
          if (!keepCachedTasks) {
            setTasks([])
            setTotal(0)
            setActiveCount(0)
            setStoppableCount(0)
          }
          setError(e instanceof Error ? e.message : String(e))
        }
      } finally {
        if (!silent && selectedEmployeeIdRef.current === employeeId) {
          setLoading(false)
        }
      }
    },
    [api, workspaceConfigured],
  )

  React.useEffect(() => {
    void refresh()
  }, [refresh, selectedEmployeeId, refreshTick, page])

  const shouldPoll =
    workspaceConfigured &&
    Boolean(selectedEmployeeId) &&
    (pollWhileBusy || activeCount > 0 || hasActiveEmployeeTasks(tasks))

  React.useEffect(() => {
    if (!shouldPoll) return
    void refresh({ silent: true })
    const timer = window.setInterval(() => {
      void refresh({ silent: true })
    }, TASK_POLL_INTERVAL_MS)
    return () => window.clearInterval(timer)
  }, [shouldPoll, refresh])

  const setPageAndRefresh = React.useCallback((nextPage: number) => {
    setPage(nextPage)
  }, [])

  return {
    tasks,
    total,
    activeCount,
    stoppableCount,
    page,
    pageSize: EMPLOYEE_TASKS_PAGE_SIZE,
    setPage: setPageAndRefresh,
    loading,
    error,
    refresh,
  }
}
