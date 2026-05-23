import * as React from 'react'
import { AgentTaskRecord, createEmployeeTasksApi } from './employeeTasksApi'

export function useEmployeeTasks(
  apiBase: string,
  locale: string,
  workspaceConfigured: boolean,
  selectedEmployeeId: string | null,
  refreshTick: number,
) {
  const api = React.useMemo(() => createEmployeeTasksApi(apiBase, locale), [apiBase, locale])
  const [tasks, setTasks] = React.useState<AgentTaskRecord[]>([])
  const [loading, setLoading] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  const selectedEmployeeIdRef = React.useRef(selectedEmployeeId)
  selectedEmployeeIdRef.current = selectedEmployeeId

  const refresh = React.useCallback(async () => {
    const employeeId = selectedEmployeeIdRef.current
    if (!workspaceConfigured || !employeeId) {
      setTasks([])
      setError(null)
      setLoading(false)
      return
    }
    setLoading(true)
    setError(null)
    try {
      const items = await api.listByExecutor(employeeId)
      if (selectedEmployeeIdRef.current !== employeeId) return
      setTasks(items)
    } catch (e) {
      if (selectedEmployeeIdRef.current !== employeeId) return
      setTasks([])
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      if (selectedEmployeeIdRef.current === employeeId) {
        setLoading(false)
      }
    }
  }, [api, workspaceConfigured])

  React.useEffect(() => {
    void refresh()
  }, [refresh, selectedEmployeeId, refreshTick])

  return { tasks, loading, error, refresh }
}
