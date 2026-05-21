import * as React from 'react'
import {
  createRequirementsApi,
  type OpinionUserAction,
  type RequirementDetail,
  type RequirementPhase,
  type RequirementReview,
  type RequirementSummary,
} from './requirementsApi'

export function useRequirementsWorkspace(
  apiBase: string,
  locale: string,
  workspaceConfigured: boolean,
  refreshTick: number,
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

  const loadDetail = React.useCallback(
    async (id: string) => {
      const data = await api.get(id)
      setDetail(data)
      void loadReview(id).catch(() => setReview(null))
      return data
    },
    [api, loadReview],
  )

  const refresh = React.useCallback(async () => {
    if (!workspaceConfigured) {
      setItems([])
      setSelectedId(null)
      setDetail(null)
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
      }
    } catch (e) {
      setItems([])
      setSelectedId(null)
      setDetail(null)
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

  return {
    items,
    selectedId,
    detail,
    review,
    loading,
    busy,
    reviewLoading,
    reviewRunning,
    reviewForcePassing,
    opinionActionKey,
    error,
    refresh,
    selectRequirement,
    createRequirement,
    saveRequirement,
    runReview,
    forcePassReview,
    opinionAction,
    loadReview,
  }
}
