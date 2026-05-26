import * as React from 'react'
import { AgentTaskRecord, createEmployeeTasksApi } from './employeeTasksApi'

const TASK_POLL_INTERVAL_MS = 2500

export function hasActiveEmployeeTasks(tasks: AgentTaskRecord[]): boolean {
  return tasks.some(
    (task) => task.status === 'pending' || task.status === 'running' || task.status === 'queued_rerun',
  )
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

  const selectedEmployeeIdRef = React.useRef(selectedEmployeeId)
  selectedEmployeeIdRef.current = selectedEmployeeId

  const refresh = React.useCallback(async (options?: { silent?: boolean }) => {
    const silent = options?.silent ?? false
    const employeeId = selectedEmployeeIdRef.current
    if (!workspaceConfigured || !employeeId) {
      setTasks([])
      setError(null)
      if (!silent) setLoading(false)
      return
    }
    if (!silent) {
      setLoading(true)
      setError(null)
    }
    try {
      const items = await api.listByExecutor(employeeId)
      if (selectedEmployeeIdRef.current !== employeeId) return
      setTasks(items)
      if (!silent) setError(null)
    } catch (e) {
      if (selectedEmployeeIdRef.current !== employeeId) return
      if (!silent) {
        setTasks([])
        setError(e instanceof Error ? e.message : String(e))
      }
    } finally {
      if (!silent && selectedEmployeeIdRef.current === employeeId) {
        setLoading(false)
      }
    }
  }, [api, workspaceConfigured])

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
