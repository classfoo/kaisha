import * as React from 'react'
import {
  createRequirementsApi,
  type AgentDispatch,
  type DevTaskStatus,
  type OpinionUserAction,
  type RequirementDetail,
  type RequirementDevelopment,
  type RequirementPhase,
  type RequirementRelease,
  type RequirementReview,
  type RequirementSummary,
  type RequirementTesting,
} from './requirementsApi'

export function useRequirementsWorkspace(
  apiBase: string,
  locale: string,
  workspaceConfigured: boolean,
  refreshTick: number,
  onAgentDispatch?: (dispatch: AgentDispatch) => void,
) {
  const api = React.useMemo(() => createRequirementsApi(apiBase, locale), [apiBase, locale])
  const [items, setItems] = React.useState<RequirementSummary[]>([])
  const [selectedId, setSelectedId] = React.useState<string | null>(null)
  const [detail, setDetail] = React.useState<RequirementDetail | null>(null)
  const [loading, setLoading] = React.useState(false)
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const [review, setReview] = React.useState<RequirementReview | null>(null)
  const [reviewLoading, setReviewLoading] = React.useState(false)
  const [reviewRunning, setReviewRunning] = React.useState(false)
  const [reviewForcePassing, setReviewForcePassing] = React.useState(false)
  const [opinionActionKey, setOpinionActionKey] = React.useState<string | null>(null)
  const [archivedItems, setArchivedItems] = React.useState<RequirementSummary[]>([])
  const [showArchived, setShowArchived] = React.useState(false)
  const [abandoningId, setAbandoningId] = React.useState<string | null>(null)
  const [reinstatingId, setReinstatingId] = React.useState<string | null>(null)
  const [hardDeletingId, setHardDeletingId] = React.useState<string | null>(null)
  const [development, setDevelopment] = React.useState<RequirementDevelopment | null>(null)
  const [devLoading, setDevLoading] = React.useState(false)
  const [devActionKey, setDevActionKey] = React.useState<string | null>(null)
  const [devStarting, setDevStarting] = React.useState(false)
  const [testing, setTesting] = React.useState<RequirementTesting | null>(null)
  const [testingLoading, setTestingLoading] = React.useState(false)
  const [release, setRelease] = React.useState<RequirementRelease | null>(null)
  const [releaseLoading, setReleaseLoading] = React.useState(false)
  const [agentActionKey, setAgentActionKey] = React.useState<string | null>(null)
  const [agentNotice, setAgentNotice] = React.useState<AgentDispatch | null>(null)

  const selectedIdRef = React.useRef(selectedId)
  selectedIdRef.current = selectedId

  const loadReview = React.useCallback(
    async (id: string) => {
      setReviewLoading(true)
      try {
        const data = await api.getReview(id)
        setReview(data)
        return data
      } catch (e) {
        setReview(null)
        throw e
      } finally {
        setReviewLoading(false)
      }
    },
    [api],
  )

  const loadDevelopment = React.useCallback(
    async (id: string, phase: RequirementPhase) => {
      if (phase !== 'development') {
        setDevelopment(null)
        setDevLoading(false)
        return null
      }
      setDevLoading(true)
      try {
        const data = await api.getDevelopment(id)
        setDevelopment(data)
        return data
      } catch (e) {
        setDevelopment(null)
        throw e
      } finally {
        setDevLoading(false)
      }
    },
    [api],
  )

  const loadTesting = React.useCallback(
    async (id: string, phase: RequirementPhase) => {
      if (phase !== 'testing') {
        setTesting(null)
        setTestingLoading(false)
        return null
      }
      setTestingLoading(true)
      try {
        const data = await api.getTesting(id)
        setTesting(data)
        return data
      } catch (e) {
        setTesting(null)
        throw e
      } finally {
        setTestingLoading(false)
      }
    },
    [api],
  )

  const loadRelease = React.useCallback(
    async (id: string, phase: RequirementPhase) => {
      if (phase !== 'release') {
        setRelease(null)
        setReleaseLoading(false)
        return null
      }
      setReleaseLoading(true)
      try {
        const data = await api.getRelease(id)
        setRelease(data)
        return data
      } catch (e) {
        setRelease(null)
        throw e
      } finally {
        setReleaseLoading(false)
      }
    },
    [api],
  )

  const loadDetail = React.useCallback(
    async (id: string) => {
      const data = await api.get(id)
      setDetail(data)
      // Note: agentNotice is no longer cleared here to allow persistent feedback
      void loadReview(id).catch(() => setReview(null))
      void loadDevelopment(id, data.phase).catch(() => setDevelopment(null))
      void loadTesting(id, data.phase).catch(() => setTesting(null))
      void loadRelease(id, data.phase).catch(() => setRelease(null))
      return data
    },
    [api, loadReview, loadDevelopment, loadTesting, loadRelease],
  )

  const refresh = React.useCallback(async () => {
    if (!workspaceConfigured) {
      setItems([])
      setSelectedId(null)
      setDetail(null)
      setDevelopment(null)
      setError(null)
      return
    }
    setLoading(true)
    setError(null)
    try {
      const list = await api.list()
      setItems(list)
      const pick =
        list.find((r) => r.id === selectedIdRef.current) ?? list[0] ?? null
      if (pick) {
        setSelectedId(pick.id)
        await loadDetail(pick.id)
      } else {
        setSelectedId(null)
        setDetail(null)
        setDevelopment(null)
      }
    } catch (e) {
      setItems([])
      setSelectedId(null)
      setDetail(null)
      setDevelopment(null)
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [api, loadDetail, workspaceConfigured])

  React.useEffect(() => {
    void refresh()
  }, [refresh, refreshTick])

  const selectRequirement = React.useCallback(
    async (id: string) => {
      setSelectedId(id)
      setError(null)
      try {
        await loadDetail(id)
      } catch (e) {
        setDetail(null)
        setError(e instanceof Error ? e.message : String(e))
      }
    },
    [loadDetail],
  )

  const createRequirement = React.useCallback(
    async (title: string, phase?: RequirementPhase) => {
      setBusy(true)
      setError(null)
      try {
        const created = await api.create({ title, phase })
        await refresh()
        setSelectedId(created.id)
        setDetail(created)
        return created
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setBusy(false)
      }
    },
    [api, refresh],
  )

  const saveRequirement = React.useCallback(
    async (payload: { title?: string; phase?: RequirementPhase; content?: string }) => {
      if (!selectedIdRef.current) return null
      setBusy(true)
      setError(null)
      try {
        const updated = await api.update(selectedIdRef.current, payload)
        setDetail(updated)
        setItems((prev) =>
          prev.map((item) =>
            item.id === updated.id
              ? {
                  id: updated.id,
                  title: updated.title,
                  phase: updated.phase,
                  created_at_ms: updated.created_at_ms,
                  updated_at_ms: updated.updated_at_ms,
                  dir_path: updated.dir_path,
                }
              : item,
          ),
        )
        return updated
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setBusy(false)
      }
    },
    [api],
  )

  const runReview = React.useCallback(
    async (id: string) => {
      setReviewRunning(true)
      setError(null)
      try {
        let current = await api.runReview(id)
        setReview(current)
        while (current.status === 'in_progress') {
          await new Promise((resolve) => setTimeout(resolve, 2000))
          const polled = await api.getReview(id)
          if (!polled) break
          current = polled
          setReview(polled)
        }
        await loadDetail(id)
        return current
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setReviewRunning(false)
      }
    },
    [api, loadDetail],
  )

  React.useEffect(() => {
    if (!selectedId || review?.status !== 'in_progress' || reviewRunning) return
    const timer = window.setInterval(() => {
      void loadReview(selectedId).catch(() => undefined)
    }, 2000)
    return () => window.clearInterval(timer)
  }, [selectedId, review?.status, reviewRunning, loadReview])

  const opinionAction = React.useCallback(
    async (requirementId: string, employeeId: string, action: OpinionUserAction) => {
      const key = `${employeeId}:${action}`
      setOpinionActionKey(key)
      setError(null)
      try {
        const result = await api.opinionAction(requirementId, employeeId, action)
        setReview(result)
        if (action === 'rerun') {
          for (let i = 0; i < 120; i++) {
            await new Promise((resolve) => setTimeout(resolve, 2000))
            const polled = await api.getReview(requirementId)
            if (!polled) break
            setReview(polled)
            const row = polled.opinions.find((o) => o.employee_id === employeeId)
            if (row && row.status !== 'in_progress' && row.status !== 'revising') break
          }
        }
        await loadDetail(requirementId)
        return result
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setOpinionActionKey(null)
      }
    },
    [api, loadDetail],
  )

  const forcePassReview = React.useCallback(
    async (id: string) => {
      setReviewForcePassing(true)
      setError(null)
      try {
        const result = await api.forcePassReview(id)
        setReview(result)
        const updated = await loadDetail(id)
        return { review: result, detail: updated }
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setReviewForcePassing(false)
      }
    },
    [api, loadDetail],
  )

  const abandonRequirement = React.useCallback(
    async (id: string) => {
      setAbandoningId(id)
      setError(null)
      try {
        const updated = await api.abandon(id)
        setItems((prev) => prev.filter((r) => r.id !== id))
        setArchivedItems((prev) => {
          const summary: RequirementSummary = {
            id: updated.id,
            title: updated.title,
            phase: updated.phase,
            created_at_ms: updated.created_at_ms,
            updated_at_ms: updated.updated_at_ms,
            dir_path: updated.dir_path,
          }
          const existing = prev.find((s) => s.id === summary.id)
          if (existing) {
            return prev.map((s) => s.id === summary.id ? summary : s)
          }
          const next = [...prev, summary]
          next.sort((a, b) => b.updated_at_ms - a.updated_at_ms)
          return next
        })
        if (selectedIdRef.current === id) {
          setSelectedId(null)
          setDetail(null)
        }
        return updated
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setAbandoningId(null)
      }
    },
    [api],
  )

  const startDevelopmentAction = React.useCallback(
    async (id: string) => {
      setDevStarting(true)
      setError(null)
      try {
        const updated = await api.startDevelopment(id)
        setDevelopment(updated)
        await loadDetail(id)
        return updated
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setDevStarting(false)
      }
    },
    [api, loadDetail],
  )

  const createDevTaskAction = React.useCallback(
    async (id: string, payload: { title: string; assignee?: string }) => {
      setDevActionKey('create')
      setError(null)
      try {
        const updated = await api.createDevTask(id, payload)
        setDevelopment(updated)
        await loadDetail(id)
        return updated
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setDevActionKey(null)
      }
    },
    [api, loadDetail],
  )

  const updateDevTaskAction = React.useCallback(
    async (id: string, taskId: string, payload: { title?: string; assignee?: string; progress?: number }) => {
      setDevActionKey(`${taskId}:update`)
      setError(null)
      try {
        const updated = await api.updateDevTask(id, taskId, payload)
        setDevelopment(updated)
        return updated
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setDevActionKey(null)
      }
    },
    [api],
  )

  const deleteDevTaskAction = React.useCallback(
    async (id: string, taskId: string) => {
      setDevActionKey(`${taskId}:delete`)
      setError(null)
      try {
        const updated = await api.deleteDevTask(id, taskId)
        setDevelopment(updated)
        return updated
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setDevActionKey(null)
      }
    },
    [api],
  )

  const devTaskAction = React.useCallback(
    async (id: string, taskId: string, action: string) => {
      setDevActionKey(`${taskId}:${action}`)
      setError(null)
      try {
        const updated = await api.devTaskAction(id, taskId, action)
        setDevelopment(updated)
        await loadDetail(id)
        return updated
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setDevActionKey(null)
      }
    },
    [api, loadDetail],
  )

  const optimizeAction = React.useCallback(
    async (id: string) => {
      setAgentActionKey('optimize')
      setError(null)
      try {
        const dispatch = await api.optimize(id)
        setAgentNotice(dispatch)
        onAgentDispatch?.(dispatch)
        return dispatch
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setAgentActionKey(null)
      }
    },
    [api, onAgentDispatch],
  )

  const splitDevTasksAction = React.useCallback(
    async (id: string) => {
      setAgentActionKey('splitDev')
      setError(null)
      try {
        const dispatch = await api.splitDevTasks(id)
        setAgentNotice(dispatch)
        onAgentDispatch?.(dispatch)
        return dispatch
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setAgentActionKey(null)
      }
    },
    [api, onAgentDispatch],
  )

  const splitTestTasksAction = React.useCallback(
    async (id: string) => {
      setAgentActionKey('splitTest')
      setError(null)
      try {
        const dispatch = await api.splitTestTasks(id)
        setAgentNotice(dispatch)
        onAgentDispatch?.(dispatch)
        return dispatch
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setAgentActionKey(null)
      }
    },
    [api, onAgentDispatch],
  )

  const testTaskActionFn = React.useCallback(
    async (id: string, taskId: string, action: string) => {
      setAgentActionKey(`${taskId}:${action}`)
      setError(null)
      try {
        const updated = await api.testTaskAction(id, taskId, action)
        setTesting(updated)
        return updated
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setAgentActionKey(null)
      }
    },
    [api],
  )

  const reloadTesting = React.useCallback(
    async (id: string) => {
      try {
        const data = await api.getTesting(id)
        setTesting(data)
        return data
      } catch {
        return null
      }
    },
    [api],
  )

  const packageReleaseAction = React.useCallback(
    async (id: string) => {
      setAgentActionKey('package')
      setError(null)
      try {
        const dispatch = await api.packageRelease(id)
        setAgentNotice(dispatch)
        onAgentDispatch?.(dispatch)
        return dispatch
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setAgentActionKey(null)
      }
    },
    [api, onAgentDispatch],
  )

  const startReleaseAction = React.useCallback(
    async (id: string) => {
      setAgentActionKey('start')
      setError(null)
      try {
        const dispatch = await api.startRelease(id)
        setAgentNotice(dispatch)
        onAgentDispatch?.(dispatch)
        return dispatch
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setAgentActionKey(null)
      }
    },
    [api, onAgentDispatch],
  )

  const reloadRelease = React.useCallback(
    async (id: string) => {
      setReleaseLoading(true)
      try {
        const data = await api.getRelease(id)
        setRelease(data)
        return data
      } catch {
        return null
      } finally {
        setReleaseLoading(false)
      }
    },
    [api],
  )

  const clearAgentNotice = React.useCallback(() => {
    setAgentNotice(null)
  }, [])

  // Load archived requirements
  React.useEffect(() => {
    if (!workspaceConfigured) {
      setArchivedItems([])
      return
    }
    let cancelled = false
    const headers = { 'x-lang': locale }
    api.listArchived()
      .then((list) => {
        if (!cancelled) setArchivedItems(list)
      })
      .catch(() => {
        if (!cancelled) setArchivedItems([])
      })
    return () => { cancelled = true }
  }, [api, locale, workspaceConfigured, refreshTick])

  const reinstateRequirement = React.useCallback(
    async (id: string) => {
      setReinstatingId(id)
      setError(null)
      try {
        const updated = await api.reinstate(id)
        setArchivedItems((prev) => prev.filter((r) => r.id !== id))
        setItems((prev) => {
          const summary: RequirementSummary = {
            id: updated.id,
            title: updated.title,
            phase: updated.phase,
            created_at_ms: updated.created_at_ms,
            updated_at_ms: updated.updated_at_ms,
            dir_path: updated.dir_path,
          }
          const next = [...prev, summary]
          next.sort((a, b) => b.updated_at_ms - a.updated_at_ms)
          return next
        })
        return updated
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setReinstatingId(null)
      }
    },
    [api],
  )

  const hardDeleteRequirement = React.useCallback(
    async (id: string) => {
      setHardDeletingId(id)
      setError(null)
      try {
        await api.hardDelete(id)
        setArchivedItems((prev) => prev.filter((r) => r.id !== id))
        if (selectedIdRef.current === id) {
          setSelectedId(null)
          setDetail(null)
        }
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e)
        setError(msg)
        throw e
      } finally {
        setHardDeletingId(null)
      }
    },
    [api],
  )

  return {
    items,
    selectedId,
    detail,
    review,
    development,
    loading,
    busy,
    reviewLoading,
    reviewRunning,
    reviewForcePassing,
    opinionActionKey,
    archivedItems,
    showArchived,
    setShowArchived,
    abandoningId,
    reinstatingId,
    hardDeletingId,
    devLoading,
    devActionKey,
    devStarting,
    testing,
    testingLoading,
    release,
    releaseLoading,
    agentActionKey,
    agentNotice,
    clearAgentNotice,
    error,
    refresh,
    selectRequirement,
    createRequirement,
    saveRequirement,
    runReview,
    forcePassReview,
    opinionAction,
    loadReview,
    abandonRequirement,
    reinstateRequirement,
    hardDeleteRequirement,
    loadDevelopment,
    startDevelopmentAction,
    createDevTaskAction,
    updateDevTaskAction,
    deleteDevTaskAction,
    devTaskAction,
    optimizeAction,
    splitDevTasksAction,
    loadTesting,
    reloadTesting,
    splitTestTasksAction,
    testTaskAction: testTaskActionFn,
    loadRelease,
    reloadRelease,
    packageReleaseAction,
    startReleaseAction,
  }
}
