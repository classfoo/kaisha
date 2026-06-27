import React from 'react'
import { Locale, resolveLocale, t } from './i18n'
import { EmployeeDirectoryRecord } from './components/EmployeeList'
import { LeftSidebar, NavMenu } from './components/LeftSidebar'
import { useGitWorkspace } from './features/git/useGitWorkspace'
import { useRequirementsWorkspace } from './features/requirements/useRequirementsWorkspace'
import type { AgentDispatch, RequirementPhase } from './features/requirements/requirementsApi'
import { LeftPanel } from './components/LeftPanel'
import { useEmployeeTasks } from './features/employee-tasks/useEmployeeTasks'
import { createEmployeeTasksApi } from './features/employee-tasks/employeeTasksApi'
import { WorkArea } from './components/WorkArea'
import { SettingsCards } from './components/SettingsCards'

const API_BASE = import.meta.env.VITE_API_BASE ?? 'http://127.0.0.1:8080'

const CHAT_IDENTITY_STORAGE_KEY = 'kaisha.chatIdentity'

type ChatIdentityDraft = { displayName: string; avatarUrl: string }

function readChatIdentityDraft(): ChatIdentityDraft {
  if (typeof window === 'undefined') return { displayName: '', avatarUrl: '' }
  try {
    const raw = window.localStorage.getItem(CHAT_IDENTITY_STORAGE_KEY)
    if (!raw) return { displayName: '', avatarUrl: '' }
    const j = JSON.parse(raw) as { displayName?: unknown; avatarUrl?: unknown }
    return {
      displayName: typeof j.displayName === 'string' ? j.displayName : '',
      avatarUrl: typeof j.avatarUrl === 'string' ? j.avatarUrl : '',
    }
  } catch {
    return { displayName: '', avatarUrl: '' }
  }
}
type WorkspaceStatus = {
  configured: boolean
  path: string | null
  source: 'env' | 'config' | 'unset'
}
type SettingsSection = 'tools' | 'departments' | 'roles' | 'employees' | 'work_rules' | 'language'
type ToolKind =
  | 'claude_code'
  | 'qwen_code'
  | 'qoder_cli'
  | 'cursor_cli'
  | 'kimi_cli'
  | 'codex'
type ToolCatalogItem = {
  kind: ToolKind
  display_name: string
  schema: { title: string; fields: { key: string; label: string; field_type: 'text' | 'number' | 'boolean' | 'select' | 'combobox' | 'password'; required: boolean; options: string[]; placeholder?: string }[] }
}
type ToolInstance = {
  id: string
  kind: ToolKind
  name: string
  enabled: boolean
  version: number
  config: Record<string, unknown>
}
type DepartmentItem = { id: number; name: string; lead: string }
type RoleItem = { id: number; name: string; level: 'junior' | 'mid' | 'senior' }
type EmployeeItem = { id: number; name: string; department: string; role: string }

export default function App() {
  const [locale, setLocale] = React.useState<Locale>(() =>
    resolveLocale(window.localStorage.getItem('kaisha.locale') ?? navigator.language),
  )
  const [status, setStatus] = React.useState('checking...')
  const [workspace, setWorkspace] = React.useState<WorkspaceStatus | null>(null)
  const [workspaceInput, setWorkspaceInput] = React.useState('')
  const [workspaceError, setWorkspaceError] = React.useState('')
  const [savingWorkspace, setSavingWorkspace] = React.useState(false)
  const [settingsOpen, setSettingsOpen] = React.useState(false)
  const [shopOpen, setShopOpen] = React.useState(true)
  const [settingsSection, setSettingsSection] = React.useState<SettingsSection>('tools')
  const [toolCatalog, setToolCatalog] = React.useState<ToolCatalogItem[]>([])
  const [toolInstances, setToolInstances] = React.useState<ToolInstance[]>([])
  const [toolKindDraft, setToolKindDraft] = React.useState<ToolKind>('claude_code')
  const [activeToolId, setActiveToolId] = React.useState<string | null>(null)
  const [toolNameDraft, setToolNameDraft] = React.useState('')
  const [toolEnabledDraft, setToolEnabledDraft] = React.useState(true)
  const [toolConfigDraft, setToolConfigDraft] = React.useState<Record<string, unknown>>({})
  const [toolSaving, setToolSaving] = React.useState(false)
  const [togglingToolIds, setTogglingToolIds] = React.useState<ReadonlySet<string>>(() => new Set())
  const [toolError, setToolError] = React.useState('')
  const [departmentForm, setDepartmentForm] = React.useState({ name: '', lead: '' })
  const [roleForm, setRoleForm] = React.useState<RoleItem['level']>('mid')
  const [roleName, setRoleName] = React.useState('')
  const [employeeForm, setEmployeeForm] = React.useState({
    name: '',
    department: '',
    role: '',
  })
  const [employeeDirectory, setEmployeeDirectory] = React.useState<EmployeeDirectoryRecord[]>([])
  const [selectedEmployeeId, setSelectedEmployeeId] = React.useState<string | null>(null)
  const [archivedEmployees, setArchivedEmployees] = React.useState<EmployeeDirectoryRecord[]>([])
  const [showArchived, setShowArchived] = React.useState(false)
  const [messageDraft, setMessageDraft] = React.useState('')
  const [chatIdentityDraft, setChatIdentityDraft] = React.useState<ChatIdentityDraft>(() => readChatIdentityDraft())
  const [departments, setDepartments] = React.useState<DepartmentItem[]>([])
  const [roles, setRoles] = React.useState<RoleItem[]>([])
  const [creatingEmployee, setCreatingEmployee] = React.useState(false)
  const [employeeCreateError, setEmployeeCreateError] = React.useState('')
  const [deletingEmployeeId, setDeletingEmployeeId] = React.useState<string | null>(null)
  const [reinstateEmployeeId, setReinstateEmployeeId] = React.useState<string | null>(null)
  const [handoverEmployeeId, setHandoverEmployeeId] = React.useState<string | null>(null)
  const [hardDeletingEmployeeId, setHardDeletingEmployeeId] = React.useState<string | null>(null)
  const [activeNav, setActiveNav] = React.useState<NavMenu>('chat')
  const [refreshTick, setRefreshTick] = React.useState(0)
  const [sidePanelWidth, setSidePanelWidth] = React.useState(260)
  const [resizingPanel, setResizingPanel] = React.useState(false)
  const [employeeTasksRefreshing, setEmployeeTasksRefreshing] = React.useState(false)
  const resizeStartXRef = React.useRef(0)
  const resizeStartWidthRef = React.useRef(260)
  const tt = React.useCallback((key: string) => t(locale, key), [locale])
  const chatSenderProfile = React.useMemo(
    () => ({
      name: chatIdentityDraft.displayName.trim() || tt('ui.chat.senderDefaultName'),
      avatarUrl: chatIdentityDraft.avatarUrl.trim(),
    }),
    [chatIdentityDraft.avatarUrl, chatIdentityDraft.displayName, tt],
  )
  const topNavItems: { id: NavMenu; labelKey: string }[] = [
    { id: 'home', labelKey: 'ui.nav.home' },
    { id: 'chat', labelKey: 'ui.nav.chat' },
    { id: 'git', labelKey: 'ui.nav.git' },
    { id: 'requirements', labelKey: 'ui.nav.requirements' },
  ]
  const bottomNavItems: { id: 'settings'; labelKey: string }[] = [
    { id: 'settings', labelKey: 'ui.actions.settings' },
  ]
  const git = useGitWorkspace(API_BASE, locale, Boolean(workspace?.configured), refreshTick)
  // Ref to track active polling timer for agent dispatch
  const agentDispatchPollTimerRef = React.useRef<ReturnType<typeof setInterval> | null>(null)

  const handleAgentDispatch = React.useCallback((dispatch: AgentDispatch) => {
    // Clear any existing poll timer from a previous dispatch
    if (agentDispatchPollTimerRef.current) {
      clearInterval(agentDispatchPollTimerRef.current)
    }

    // Switch to the assigned employee and refresh their task list immediately.
    // The backend now creates the agent task synchronously before responding.
    setSelectedEmployeeId(dispatch.employee_id)
    setRefreshTick((t) => t + 1)
    // Switch to chat view so user sees the task progress
    setActiveNav('chat')
    // Refresh chat messages to show the new task_process message
    setChatMessagesRefreshTick((t) => t + 1)

    // The backend spawns the agent task asynchronously, so the task_process
    // message may not be written to conversation.json yet when this callback
    // fires. Poll a few times to ensure the message eventually appears.
    let attempt = 0
    const pollTimer = setInterval(() => {
      attempt += 1
      setChatMessagesRefreshTick((t) => t + 1)
      if (attempt >= 4) {
        agentDispatchPollTimerRef.current = null
        clearInterval(pollTimer)
      }
    }, 500)
    agentDispatchPollTimerRef.current = pollTimer
  }, [])

  const requirements = useRequirementsWorkspace(API_BASE, locale, Boolean(workspace?.configured), refreshTick, handleAgentDispatch)
  const [employeeTasksExploring, setEmployeeTasksExploring] = React.useState(false)
  const [chatMessagesRefreshTick, setChatMessagesRefreshTick] = React.useState(0)
  const [employeeTasksExploreError, setEmployeeTasksExploreError] = React.useState<string | null>(null)
  const [rerunningTaskId, setRerunningTaskId] = React.useState<string | null>(null)
  const [employeeTaskRerunError, setEmployeeTaskRerunError] = React.useState<string | null>(null)
  const [stoppingTaskId, setStoppingTaskId] = React.useState<string | null>(null)
  const [stoppingAllEmployeeTasks, setStoppingAllEmployeeTasks] = React.useState(false)
  const [employeeTaskStopError, setEmployeeTaskStopError] = React.useState<string | null>(null)
  const employeeTasks = useEmployeeTasks(
    API_BASE,
    locale,
    Boolean(workspace?.configured),
    selectedEmployeeId,
    refreshTick,
    employeeTasksExploring || rerunningTaskId != null || stoppingTaskId != null || stoppingAllEmployeeTasks,
  )
  const employeeTasksApi = React.useMemo(
    () => createEmployeeTasksApi(API_BASE, locale),
    [locale],
  )

  React.useEffect(() => {
    setEmployeeTasksExploreError(null)
    setEmployeeTaskRerunError(null)
    setEmployeeTaskStopError(null)
  }, [selectedEmployeeId])

  const formatEmployeeTasksListError = React.useCallback(
    (raw: string | null) => {
      if (!raw) return null
      const lower = raw.toLowerCase()
      if (
        lower.includes('load failed') ||
        lower.includes('failed to fetch') ||
        lower.includes('networkerror') ||
        lower.includes('network')
      ) {
        return tt('ui.employeeTasks.listError')
      }
      return raw
    },
    [tt],
  )

  const runEmployeeExplore = React.useCallback(async () => {
    if (!workspace?.configured || !selectedEmployeeId || employeeTasksExploring) return
    setEmployeeTasksExploring(true)
    setEmployeeTasksExploreError(null)
    try {
      await employeeTasksApi.triggerExplore(selectedEmployeeId)
      void employeeTasks.refresh({ silent: true })
      // Signal chat panel to refresh since the explore task writes to conversation.json
      setChatMessagesRefreshTick((t) => t + 1)
    } catch (err) {
      setEmployeeTasksExploreError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeTasks.exploreError'),
      )
    } finally {
      setEmployeeTasksExploring(false)
    }
  }, [
    workspace?.configured,
    selectedEmployeeId,
    employeeTasksExploring,
    employeeTasksApi,
    employeeTasks.refresh,
    tt,
  ])

  const rerunEmployeeTask = React.useCallback(async (taskId: string) => {
    if (!workspace?.configured || rerunningTaskId === taskId) return
    setRerunningTaskId(taskId)
    setEmployeeTaskRerunError(null)
    try {
      await employeeTasksApi.rerun(taskId)
      void employeeTasks.refresh()
      // Signal chat panel to refresh since the rerun streams into conversation.json
      setChatMessagesRefreshTick((t) => t + 1)
    } catch (err) {
      setEmployeeTaskRerunError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeTasks.rerunError'),
      )
    } finally {
      setRerunningTaskId(null)
    }
  }, [workspace?.configured, rerunningTaskId, employeeTasksApi, employeeTasks.refresh, tt])

  const stopEmployeeTask = React.useCallback(async (taskId: string) => {
    if (!workspace?.configured || stoppingTaskId || stoppingAllEmployeeTasks) return
    setStoppingTaskId(taskId)
    setEmployeeTaskStopError(null)
    try {
      await employeeTasksApi.stop(taskId)
      void employeeTasks.refresh()
    } catch (err) {
      setEmployeeTaskStopError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeTasks.stopError'),
      )
    } finally {
      setStoppingTaskId(null)
    }
  }, [workspace?.configured, stoppingTaskId, stoppingAllEmployeeTasks, employeeTasksApi, employeeTasks.refresh, tt])

  const stopAllEmployeeTasks = React.useCallback(async () => {
    if (!workspace?.configured || !selectedEmployeeId || stoppingAllEmployeeTasks || stoppingTaskId) return
    setStoppingAllEmployeeTasks(true)
    setEmployeeTaskStopError(null)
    try {
      await employeeTasksApi.stopAll(selectedEmployeeId)
      void employeeTasks.refresh()
    } catch (err) {
      setEmployeeTaskStopError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeTasks.stopAllError'),
      )
    } finally {
      setStoppingAllEmployeeTasks(false)
    }
  }, [
    workspace?.configured,
    selectedEmployeeId,
    stoppingAllEmployeeTasks,
    stoppingTaskId,
    employeeTasksApi,
    employeeTasks.refresh,
    tt,
  ])

  const fetchEmployeeTaskDetail = React.useCallback(
    (taskId: string) => employeeTasksApi.getDetail(taskId),
    [employeeTasksApi],
  )

  const [newGitRepoName, setNewGitRepoName] = React.useState('')
  const [newRequirementTitle, setNewRequirementTitle] = React.useState('')
  const requirementPhaseLabel = React.useCallback(
    (phase: RequirementPhase) => tt(`ui.requirements.phases.${phase}`),
    [tt],
  )

  React.useEffect(() => {
    fetch(`${API_BASE}/api/health`)
      .then((res) => res.json())
      .then((json) => setStatus(json.status ?? 'unknown'))
      .catch(() => setStatus('offline'))

    fetch(`${API_BASE}/api/workspace`)
      .then((res) => res.json())
      .then((json: WorkspaceStatus) => {
        setWorkspace(json)
        if (json.path) {
          setWorkspaceInput(json.path)
        }
      })
      .catch(() => setWorkspaceError(tt('ui.workspace.loadError')))
  }, [refreshTick])

  React.useEffect(() => {
    if (!workspace?.configured) {
      setEmployeeDirectory([])
      setSelectedEmployeeId(null)
      setArchivedEmployees([])
      return
    }

    let cancelled = false
    let retryTimer: number | undefined
    let retries = 0
    const headers = { 'x-lang': locale }
    const loadEmployees = () => {
      fetch(`${API_BASE}/api/employees`, { headers })
        .then((res) => {
          if (!res.ok) throw new Error('load employees failed')
          return res.json()
        })
        .then((json: EmployeeDirectoryRecord[]) => {
          if (cancelled) return
          setEmployeeDirectory(json)
          if (json.length > 0) {
            setSelectedEmployeeId((prev) => prev ?? json[0].id)
          } else {
            setSelectedEmployeeId(null)
          }
        })
        .catch(() => {
          if (cancelled) return
          if (retries < 4) {
            retries += 1
            retryTimer = window.setTimeout(loadEmployees, 500)
            return
          }
          setEmployeeDirectory([])
          setSelectedEmployeeId(null)
        })
    }

    const loadArchived = () => {
      fetch(`${API_BASE}/api/employees/archived`, { headers })
        .then((res) => {
          if (!res.ok) return
          return res.json()
        })
        .then((json: EmployeeDirectoryRecord[]) => {
          if (!cancelled) setArchivedEmployees(json ?? [])
        })
        .catch(() => {
          // silently ignore
        })
    }

    loadEmployees()
    loadArchived()

    return () => {
      cancelled = true
      if (retryTimer !== undefined) {
        window.clearTimeout(retryTimer)
      }
    }
  }, [workspace?.configured, workspace?.path, locale, refreshTick])

  React.useEffect(() => {
    if (!resizingPanel) return
    const minWidth = 220
    const maxWidth = 520

    const onMouseMove = (event: MouseEvent) => {
      const delta = event.clientX - resizeStartXRef.current
      const nextWidth = Math.min(maxWidth, Math.max(minWidth, resizeStartWidthRef.current + delta))
      setSidePanelWidth(nextWidth)
    }

    const onMouseUp = () => {
      setResizingPanel(false)
    }

    document.body.style.cursor = 'col-resize'
    window.addEventListener('mousemove', onMouseMove)
    window.addEventListener('mouseup', onMouseUp)
    return () => {
      document.body.style.cursor = ''
      window.removeEventListener('mousemove', onMouseMove)
      window.removeEventListener('mouseup', onMouseUp)
    }
  }, [resizingPanel])

  React.useEffect(() => {
    if (!settingsOpen || settingsSection !== 'tools') return
    const headers = { 'x-lang': locale }
    Promise.all([
      fetch(`${API_BASE}/api/tools/catalog`, { headers }).then((res) => res.json()),
      fetch(`${API_BASE}/api/tools/instances`, { headers }).then((res) => res.json()),
    ])
      .then(([catalog, instances]: [ToolCatalogItem[], ToolInstance[]]) => {
        setToolCatalog(catalog)
        setToolInstances(instances)
        if (catalog.length > 0) {
          setToolKindDraft(catalog[0].kind)
        }
        if (instances.length > 0) {
          const first = instances[0]
          setActiveToolId(first.id)
          setToolNameDraft(first.name)
          setToolEnabledDraft(first.enabled)
          setToolConfigDraft(first.config ?? {})
        }
      })
      .catch(() => setToolError(tt('ui.settings.tools.loadError')))
  }, [settingsOpen, settingsSection, locale, tt])

  React.useEffect(() => {
    window.localStorage.setItem('kaisha.locale', locale)
    if (workspace?.configured) {
      void fetch(`${API_BASE}/api/health`, { headers: { 'x-lang': locale } })
    }
  }, [locale, workspace?.configured])

  React.useEffect(() => {
    // Fetch shop status on mount
    fetch(`${API_BASE}/api/shop/status`, {
      headers: { 'x-lang': locale },
    })
      .then((res) => res.json())
      .then((data) => {
        if (data && typeof data.is_open === 'boolean') {
          setShopOpen(data.is_open)
        }
      })
      .catch(() => {
        // ignore network errors
      })
  }, [locale])

  React.useEffect(() => {
    window.localStorage.setItem(CHAT_IDENTITY_STORAGE_KEY, JSON.stringify(chatIdentityDraft))
  }, [chatIdentityDraft])

  const saveWorkspace = React.useCallback(async () => {
    if (!workspaceInput.trim()) {
      setWorkspaceError(tt('ui.workspace.emptyError'))
      return
    }

    setSavingWorkspace(true)
    setWorkspaceError('')
    try {
      const response = await fetch(`${API_BASE}/api/workspace`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ path: workspaceInput.trim() }),
      })
      if (!response.ok) {
        const errText = await response.text()
        throw new Error(errText || `HTTP ${response.status}`)
      }

      const data: WorkspaceStatus = await response.json()
      setWorkspace(data)
    } catch (err) {
      setWorkspaceError(
        err instanceof Error ? err.message : tt('ui.workspace.saveError'),
      )
    } finally {
      setSavingWorkspace(false)
    }
  }, [workspaceInput])

  const nextId = React.useRef(1)

  const createTool = React.useCallback(async () => {
    setToolError('')
    try {
      const response = await fetch(`${API_BASE}/api/tools/instances`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'x-lang': locale },
        body: JSON.stringify({ kind: toolKindDraft }),
      })
      if (!response.ok) throw new Error(await response.text())
      const created: ToolInstance = await response.json()
      const next = [...toolInstances, created]
      setToolInstances(next)
      setActiveToolId(created.id)
      setToolNameDraft(created.name)
      setToolEnabledDraft(created.enabled)
      setToolConfigDraft(created.config ?? {})
    } catch (err) {
      setToolError(err instanceof Error ? err.message : tt('ui.settings.tools.saveError'))
    }
  }, [locale, toolKindDraft, toolInstances, tt])

  const saveActiveTool = React.useCallback(async () => {
    if (!activeToolId) return
    setToolSaving(true)
    setToolError('')
    try {
      const response = await fetch(`${API_BASE}/api/tools/instances/${activeToolId}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json', 'x-lang': locale },
        body: JSON.stringify({
          name: toolNameDraft,
          enabled: toolEnabledDraft,
          config: toolConfigDraft,
        }),
      })
      if (!response.ok) throw new Error(await response.text())
      const updated: ToolInstance = await response.json()
      setToolInstances((prev) => prev.map((item) => (item.id === updated.id ? updated : item)))
    } catch (err) {
      setToolError(err instanceof Error ? err.message : tt('ui.settings.tools.saveError'))
    } finally {
      setToolSaving(false)
    }
  }, [activeToolId, locale, toolNameDraft, toolEnabledDraft, toolConfigDraft, tt])

  const toggleToolEnabled = React.useCallback(async (id: string, enabled: boolean) => {
    setToolError('')
    setTogglingToolIds((prev) => new Set(prev).add(id))
    setToolInstances((prev) =>
      prev.map((item) => (item.id === id ? { ...item, enabled } : item)),
    )
    if (activeToolId === id) {
      setToolEnabledDraft(enabled)
    }
    try {
      const response = await fetch(`${API_BASE}/api/tools/instances/${id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json', 'x-lang': locale },
        body: JSON.stringify({ enabled }),
      })
      if (!response.ok) throw new Error(await response.text())
      const updated: ToolInstance = await response.json()
      setToolInstances((prev) => prev.map((item) => (item.id === updated.id ? updated : item)))
      if (activeToolId === updated.id) {
        setToolEnabledDraft(updated.enabled)
      }
    } catch (err) {
      setToolInstances((prev) =>
        prev.map((item) => (item.id === id ? { ...item, enabled: !enabled } : item)),
      )
      if (activeToolId === id) {
        setToolEnabledDraft(!enabled)
      }
      setToolError(err instanceof Error ? err.message : tt('ui.settings.tools.saveError'))
    } finally {
      setTogglingToolIds((prev) => {
        const next = new Set(prev)
        next.delete(id)
        return next
      })
    }
  }, [activeToolId, locale, tt])

  const addDepartment = React.useCallback(() => {
    if (!departmentForm.name.trim() || !departmentForm.lead.trim()) return
    setDepartments((prev) => [
      ...prev,
      {
        id: nextId.current++,
        name: departmentForm.name.trim(),
        lead: departmentForm.lead.trim(),
      },
    ])
    setDepartmentForm({ name: '', lead: '' })
  }, [departmentForm.name, departmentForm.lead])

  const addRole = React.useCallback(() => {
    if (!roleName.trim()) return
    setRoles((prev) => [
      ...prev,
      {
        id: nextId.current++,
        name: roleName.trim(),
        level: roleForm,
      },
    ])
    setRoleName('')
    setRoleForm('mid')
  }, [roleName, roleForm])

  const addEmployee = React.useCallback(async () => {
    console.debug('[employee:create] settings add requested', {
      workspaceConfigured: workspace?.configured ?? false,
      creatingEmployee,
      name: employeeForm.name,
      department: employeeForm.department,
      role: employeeForm.role,
    })
    if (!workspace?.configured) {
      setEmployeeCreateError(tt('ui.employeeList.workspaceRequiredError'))
      console.warn('[employee:create] settings add blocked: workspace not configured')
      return
    }
    if (creatingEmployee) {
      console.warn('[employee:create] settings add blocked: request already in flight')
      return
    }
    if (!employeeForm.name.trim() || !employeeForm.department || !employeeForm.role) {
      setEmployeeCreateError(tt('ui.employeeList.validationError'))
      console.warn('[employee:create] settings add blocked by validation/state')
      return
    }
    setCreatingEmployee(true)
    setEmployeeCreateError('')
    const headers = { 'Content-Type': 'application/json', 'x-lang': locale }
    const requestBody = {
      name: employeeForm.name.trim(),
      department: employeeForm.department,
      role: employeeForm.role,
    }

    try {
      console.debug('[employee:create] settings POST /api/employees', requestBody)
      const response = await fetch(`${API_BASE}/api/employees`, {
        method: 'POST',
        headers,
        body: JSON.stringify(requestBody),
      })
      console.debug('[employee:create] settings response status', response.status)
      if (!response.ok) {
        const text = await response.text()
        throw new Error(text || tt('ui.employeeList.createError'))
      }

      const created: EmployeeDirectoryRecord = await response.json()
      console.debug('[employee:create] settings created', created)
      setEmployeeDirectory((prev) => {
        const next = [...prev, created]
        next.sort((a, b) => a.id.localeCompare(b.id))
        return next
      })
      setSelectedEmployeeId(created.id)
      setEmployeeForm({ name: '', department: '', role: '' })
    } catch (err) {
      console.error('[employee:create] settings failed', err)
      setEmployeeCreateError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeList.createError'),
      )
    } finally {
      setCreatingEmployee(false)
    }
  }, [workspace?.configured, creatingEmployee, employeeForm, locale, tt])

  const hireEmployee = React.useCallback(async () => {
    console.debug('[employee:hire] sidebar hire requested', {
      workspaceConfigured: workspace?.configured ?? false,
      creatingEmployee,
      currentCount: employeeDirectory.length,
    })
    if (!workspace?.configured) {
      setEmployeeCreateError(tt('ui.employeeList.workspaceRequiredError'))
      console.warn('[employee:hire] sidebar add blocked: workspace not configured')
      return
    }
    if (creatingEmployee) return
    setCreatingEmployee(true)
    setEmployeeCreateError('')
    const headers = { 'x-lang': locale }

    try {
      console.debug('[employee:hire] POST /api/employees/hire')
      const response = await fetch(`${API_BASE}/api/employees/hire`, {
        method: 'POST',
        headers,
      })
      console.debug('[employee:hire] response status', response.status)
      if (!response.ok) {
        const text = await response.text()
        throw new Error(text || tt('ui.employeeList.hireError'))
      }

      const created: EmployeeDirectoryRecord = await response.json()
      console.debug('[employee:hire] created', created)
      setEmployeeDirectory((prev) => {
        const next = [...prev, created]
        next.sort((a, b) => a.id.localeCompare(b.id))
        return next
      })
      setSelectedEmployeeId(created.id)
      void employeeTasks.refresh()
    } catch (err) {
      console.error('[employee:hire] failed', err)
      setEmployeeCreateError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeList.hireError'),
      )
    } finally {
      setCreatingEmployee(false)
    }
  }, [workspace?.configured, creatingEmployee, employeeDirectory.length, locale, employeeTasks.refresh, tt])

  const fireEmployee = React.useCallback(async (id: string) => {
    if (!workspace?.configured) return
    setDeletingEmployeeId(id)
    setEmployeeCreateError('')
    const headers = { 'x-lang': locale }

    try {
      const response = await fetch(`${API_BASE}/api/employees/${id}/fire`, {
        method: 'POST',
        headers,
      })
      if (!response.ok) {
        const text = await response.text()
        throw new Error(text || tt('ui.employeeList.fireError'))
      }

      const fired = employeeDirectory.find((e) => e.id === id)
      setEmployeeDirectory((prev) => prev.filter((e) => e.id !== id))
      if (fired) setArchivedEmployees((prev) => [...prev, fired].sort((a, b) => a.id.localeCompare(b.id)))
      setSelectedEmployeeId((prev) => (prev === id ? null : prev))
    } catch (err) {
      console.error('[employee:fire] failed', err)
      setEmployeeCreateError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeList.fireError'),
      )
    } finally {
      setDeletingEmployeeId(null)
    }
  }, [workspace?.configured, locale, employeeDirectory, tt])

  const reinstateEmployee = React.useCallback(async (id: string) => {
    if (!workspace?.configured) return
    setReinstateEmployeeId(id)
    setEmployeeCreateError('')
    const headers = { 'x-lang': locale }

    try {
      const response = await fetch(`${API_BASE}/api/employees/${id}/reinstate`, {
        method: 'POST',
        headers,
      })
      if (!response.ok) {
        const text = await response.text()
        throw new Error(text || tt('ui.employeeList.reinstateError'))
      }

      const reinstated: EmployeeDirectoryRecord = await response.json()
      setArchivedEmployees((prev) => prev.filter((e) => e.id !== id))
      setEmployeeDirectory((prev) => {
        const next = [...prev, reinstated]
        next.sort((a, b) => a.id.localeCompare(b.id))
        return next
      })
      setSelectedEmployeeId(reinstated.id)
    } catch (err) {
      console.error('[employee:reinstate] failed', err)
      setEmployeeCreateError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeList.reinstateError'),
      )
    } finally {
      setReinstateEmployeeId(null)
    }
  }, [workspace?.configured, locale, tt])

  const handoverEmployee = React.useCallback(async (id: string) => {
    if (!workspace?.configured) return
    setHandoverEmployeeId(id)
    setEmployeeCreateError('')
    const headers = { 'x-lang': locale }

    try {
      const response = await fetch(`${API_BASE}/api/employees/${id}/handover`, {
        method: 'POST',
        headers,
      })
      if (!response.ok) {
        const text = await response.text()
        throw new Error(text || tt('ui.employeeList.handoverError'))
      }

      setArchivedEmployees((prev) => prev.filter((e) => e.id !== id))
      if (selectedEmployeeId === id) setSelectedEmployeeId(null)
    } catch (err) {
      console.error('[employee:handover] failed', err)
      setEmployeeCreateError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeList.handoverError'),
      )
    } finally {
      setHandoverEmployeeId(null)
    }
  }, [workspace?.configured, selectedEmployeeId, locale, tt])

  const hardDeleteEmployee = React.useCallback(async (id: string) => {
    if (!workspace?.configured) return
    setHardDeletingEmployeeId(id)
    setEmployeeCreateError('')
    const headers = { 'x-lang': locale }

    try {
      const response = await fetch(`${API_BASE}/api/employees/${id}/hard-delete`, {
        method: 'POST',
        headers,
      })
      if (!response.ok) {
        const text = await response.text()
        throw new Error(text || tt('ui.employeeList.hardDeleteError'))
      }

      setArchivedEmployees((prev) => prev.filter((e) => e.id !== id))
      if (selectedEmployeeId === id) setSelectedEmployeeId(null)
    } catch (err) {
      console.error('[employee:hardDelete] failed', err)
      setEmployeeCreateError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeList.hardDeleteError'),
      )
    } finally {
      setHardDeletingEmployeeId(null)
    }
  }, [workspace?.configured, selectedEmployeeId, locale, tt])

  const toggleShop = React.useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/shop/toggle`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'x-lang': locale },
      })
      if (res.ok) {
        const data = await res.json()
        setShopOpen(data.is_open)
      }
    } catch {
      // ignore network errors
    }
  }, [locale])

  const handleNavMenuClick = React.useCallback((menu: NavMenu) => {
    setActiveNav(menu)
    setSettingsOpen(false)
    // Only reset workspace and employee state when navigating to home.
    // For functional tabs (chat, requirements, git), preserve the current
    // context so users don't lose their selected employee or active task view.
    if (menu === 'home') {
      setWorkspace(null)
      setStatus('checking...')
      setWorkspaceError('')
      setEmployeeCreateError('')
      setEmployeeDirectory([])
      setSelectedEmployeeId(null)
      setMessageDraft('')
      setRefreshTick((prev) => prev + 1)
    }
  }, [])

  const startResizePanel = React.useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    resizeStartXRef.current = event.clientX
    resizeStartWidthRef.current = sidePanelWidth
    setResizingPanel(true)
  }, [sidePanelWidth])

  const toggleArchived = React.useCallback(() => setShowArchived((v) => !v), [])
  const toggleRequirementsArchived = React.useCallback(() => requirements.setShowArchived((v: boolean) => !v), [requirements.setShowArchived])
  const createGitRepo = React.useCallback(() => {
    const name = newGitRepoName.trim()
    if (!name) return
    void git.createRepo(name).then(() => setNewGitRepoName(''))
  }, [newGitRepoName, git.createRepo])
  const createRequirement = React.useCallback(() => {
    const title = newRequirementTitle.trim()
    if (!title) return
    void requirements.createRequirement(title).then(() => setNewRequirementTitle(''))
  }, [newRequirementTitle, requirements.createRequirement])
  const refreshEmployeeTasks = React.useCallback(() => {
    setEmployeeTasksRefreshing(true)
    const silent = employeeTasks.tasks.some(
      (task) => task.status === 'pending' || task.status === 'running' || task.status === 'queued_rerun',
    )
    void employeeTasks.refresh({ silent }).finally(() => setEmployeeTasksRefreshing(false))
  }, [employeeTasks.refresh, employeeTasks.tasks])
  const openSettings = React.useCallback(() => setSettingsOpen(true), [])
  const closeSettings = React.useCallback(() => setSettingsOpen(false), [])

  const usesSplitWorkArea =
    activeNav === 'chat' ||
    activeNav === 'git' ||
    activeNav === 'requirements'

  return (
    <div className="app-shell">
      <LeftSidebar
        activeNav={activeNav}
        topNavItems={topNavItems}
        bottomNavItems={bottomNavItems}
        settingsOpen={settingsOpen}
        shopOpen={shopOpen}
        t={tt}
        onMenuClick={handleNavMenuClick}
        onSettingsClick={() => setSettingsOpen(true)}
        onShopToggle={toggleShop}
      />
      <WorkArea
        workAreaKey={`work-area-${activeNav}-${refreshTick}`}
        activeNav={activeNav}
        splitPane={usesSplitWorkArea ? (
          <LeftPanel
            panelKey={`left-panel-${activeNav}-${refreshTick}`}
            sidePanelWidth={sidePanelWidth}
            employees={employeeDirectory}
            selectedEmployeeId={selectedEmployeeId}
            onSelectEmployee={setSelectedEmployeeId}
            onFireEmployee={(id) => void fireEmployee(id)}
            deletingEmployeeId={deletingEmployeeId}
            activeNav={activeNav}
            creatingEmployee={creatingEmployee}
            employeeCreateError={employeeCreateError}
            workspaceConfigured={Boolean(workspace?.configured)}
            status={status}
            t={tt}
            onCreateEmployee={hireEmployee}
            showArchived={showArchived}
            onToggleArchived={toggleArchived}
            archivedEmployees={archivedEmployees}
            reinstateEmployeeId={reinstateEmployeeId}
            onReinstateEmployee={reinstateEmployee}
            handoverEmployeeId={handoverEmployeeId}
            onHandoverEmployee={handoverEmployee}
            hardDeletingEmployeeId={hardDeletingEmployeeId}
            onHardDeleteEmployee={hardDeleteEmployee}
            onResizeMouseDown={startResizePanel}
            gitRepos={git.repos}
            selectedGitRepoId={git.selectedRepoId}
            onSelectGitRepo={git.selectRepo}
            newGitRepoName={newGitRepoName}
            onNewGitRepoNameChange={setNewGitRepoName}
            onCreateGitRepo={createGitRepo}
            gitBusy={git.busy}
            gitError={git.error}
            gitLoading={git.loading}
            requirements={requirements.items}
            selectedRequirementId={requirements.selectedId}
            onSelectRequirement={requirements.selectRequirement}
            newRequirementTitle={newRequirementTitle}
            onNewRequirementTitleChange={setNewRequirementTitle}
            onCreateRequirement={createRequirement}
            requirementsBusy={requirements.busy}
            requirementsError={requirements.error}
            requirementsLoading={requirements.loading}
            requirementPhaseLabel={requirementPhaseLabel}
            showRequirementsArchived={requirements.showArchived}
            onToggleRequirementsArchived={toggleRequirementsArchived}
            archivedRequirements={requirements.archivedItems}
            onAbandonRequirement={requirements.abandonRequirement}
            onReinstateRequirement={requirements.reinstateRequirement}
            onHardDeleteRequirement={requirements.hardDeleteRequirement}
            abandoningRequirementId={requirements.abandoningId}
            reinstatingRequirementId={requirements.reinstatingId}
            hardDeletingRequirementId={requirements.hardDeletingId}
            employeeTasks={employeeTasks.tasks}
            employeeTasksTotal={employeeTasks.total}
            employeeTasksPage={employeeTasks.page}
            employeeTasksPageSize={employeeTasks.pageSize}
            employeeTasksStoppableCount={employeeTasks.stoppableCount}
            onEmployeeTasksPageChange={employeeTasks.setPage}
            employeeTasksLoading={employeeTasks.loading}
            employeeTasksError={employeeTasksExploreError ?? employeeTaskRerunError ?? employeeTaskStopError ?? formatEmployeeTasksListError(employeeTasks.error)}
            employeeTasksExploring={employeeTasksExploring}
            onEmployeeTasksExplore={runEmployeeExplore}
            employeeTasksRefreshing={employeeTasksRefreshing}
            onEmployeeTasksRefresh={refreshEmployeeTasks}
            stoppingAllEmployeeTasks={stoppingAllEmployeeTasks}
            onStopAllEmployeeTasks={stopAllEmployeeTasks}
            rerunningTaskId={rerunningTaskId}
            onRerunEmployeeTask={rerunEmployeeTask}
            stoppingTaskId={stoppingTaskId}
            onStopEmployeeTask={stopEmployeeTask}
            onFetchEmployeeTaskDetail={fetchEmployeeTaskDetail}
            locale={locale}
          />
        ) : null}
        workspaceConfigured={Boolean(workspace?.configured)}
        workspacePath={workspace?.path ?? null}
        workspaceInput={workspaceInput}
        workspaceError={workspaceError}
        savingWorkspace={savingWorkspace}
        messageDraft={messageDraft}
        settingsOpen={settingsOpen}
        settingsSection={settingsSection}
        employees={employeeDirectory}
        selectedEmployeeId={selectedEmployeeId}
        apiBase={API_BASE}
        locale={locale}
        chatSenderProfile={chatSenderProfile}
        git={git}
        requirements={requirements}
        requirementPhaseLabel={requirementPhaseLabel}
        onEmployeeTasksRefresh={refreshEmployeeTasks}
        chatMessagesRefreshTick={chatMessagesRefreshTick}
        t={tt}
        onOpenSettings={openSettings}
        onSetWorkspaceInput={setWorkspaceInput}
        onSaveWorkspace={saveWorkspace}
        onMessageDraftChange={setMessageDraft}
        onCloseSettings={closeSettings}
        onSetSettingsSection={setSettingsSection}
        settingsCards={
          <SettingsCards
            apiBase={API_BASE}
            locale={locale}
            section={settingsSection}
            tt={tt}
            toolKindDraft={toolKindDraft}
            toolCatalog={toolCatalog}
            toolInstances={toolInstances}
            activeToolId={activeToolId}
            toolNameDraft={toolNameDraft}
            toolEnabledDraft={toolEnabledDraft}
            toolConfigDraft={toolConfigDraft}
            toolError={toolError}
            toolSaving={toolSaving}
            togglingToolIds={togglingToolIds}
            onCreateTool={createTool}
            onSaveTool={saveActiveTool}
            onToggleToolEnabled={(id, enabled) => void toggleToolEnabled(id, enabled)}
            onSelectToolInstance={(id, name, enabled, config) => {
              setActiveToolId(id)
              setToolNameDraft(name)
              setToolEnabledDraft(enabled)
              setToolConfigDraft(config)
            }}
            onToolKindDraftChange={setToolKindDraft}
            onToolNameDraftChange={setToolNameDraft}
            onToolEnabledDraftChange={setToolEnabledDraft}
            onToolConfigDraftChange={setToolConfigDraft}
            departmentForm={departmentForm}
            departments={departments}
            onAddDepartment={addDepartment}
            onDepartmentFormChange={setDepartmentForm}
            roleName={roleName}
            roleForm={roleForm}
            roles={roles}
            onAddRole={addRole}
            onRoleNameChange={setRoleName}
            onRoleFormChange={setRoleForm}
            employeeForm={employeeForm}
            employeeDirectory={employeeDirectory}
            employeeCreateError={employeeCreateError}
            creatingEmployee={creatingEmployee}
            onAddEmployee={addEmployee}
            onEmployeeFormChange={setEmployeeForm}
            chatIdentityDraft={chatIdentityDraft}
            onLocaleChange={setLocale}
            onChatIdentityDraftChange={setChatIdentityDraft}
          />
        }
      />
    </div>
  )
}
