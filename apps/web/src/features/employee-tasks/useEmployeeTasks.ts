import * as React from 'react'
import { AgentTaskRecord, createEmployeeTasksApi } from './employeeTasksApi'

const TASK_POLL_INTERVAL_MS = 2500
const RETRY_ATTEMPTS = 3
const RETRY_DELAY_MS = 500

export function hasActiveEmployeeTasks(tasks: AgentTaskRecord[]): boolean {
  return tasks.some(
    (task) => task.status === 'pending' || task.status === 'running' || task.status === 'queued_rerun',
  )
}

/** Determines if an error is transient (worth retrying silently). */
function isTransientError(error: Error): boolean {
  const msg = error.message.toLowerCase()
  // Network errors, server errors, and temporary states
  return (
    msg.includes('fetch') ||
    msg.includes('network') ||
    msg.includes('internal server error') ||
    msg.includes('500') ||
    msg.includes('502') ||
    msg.includes('503') ||
    msg.includes('504') ||
    msg.includes('task_load_failed') ||
    msg.includes('skipping')
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
  const [loading, setLoading] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  // Track consecutive failures to enable retry without clearing cached data
  const failureCountRef = React.useRef(0)

  const selectedEmployeeIdRef = React.useRef(selectedEmployeeId)
  selectedEmployeeIdRef.current = selectedEmployeeId

  const refresh = React.useCallback(
    async (options?: { silent?: boolean }) => {
      const silent = options?.silent ?? false
      const employeeId = selectedEmployeeIdRef.current
      if (!workspaceConfigured || !employeeId) {
        setTasks([])
        setError(null)
        if (!silent) setLoading(false)
        failureCountRef.current = 0
        return
      }
      if (!silent) {
        setLoading(true)
        setError(null)
      }
      try {
        const items = await fetchWithRetry(
          () => api.listByExecutor(employeeId),
          silent ? RETRY_ATTEMPTS : 1,
          RETRY_DELAY_MS,
        )
        if (selectedEmployeeIdRef.current !== employeeId) return
        setTasks(items)
        setError(null)
        failureCountRef.current = 0
      } catch (e) {
        if (selectedEmployeeIdRef.current !== employeeId) return
        failureCountRef.current += 1
        const isTransient = e instanceof Error && isTransientError(e)
        // For silent (background) refreshes with transient errors, keep showing cached data
        if (silent && isTransient) {
          // Don't clear tasks on transient background errors - keep showing last known good state
          return
        }
        if (!silent) {
          // Only show error to user after persistent failures
          setTasks([])
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
  }, [refresh, selectedEmployeeId, refreshTick])

  const shouldPoll =
    workspaceConfigured &&
    Boolean(selectedEmployeeId) &&
    (pollWhileBusy || hasActiveEmployeeTasks(tasks))

  React.useEffect(() => {
    if (!shouldPoll) return
    void refresh({ silent: true })
    const timer = window.setInterval(() => {
      void refresh({ silent: true })
    }, TASK_POLL_INTERVAL_MS)
    return () => window.clearInterval(timer)
  }, [shouldPoll, refresh])

  return { tasks, loading, error, refresh }
}
