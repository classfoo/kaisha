import React from 'react'
import { Locale, resolveLocale, t } from './i18n'
import { EmployeeDirectoryRecord } from './components/EmployeeList'
import { LeftSidebar, NavMenu } from './components/LeftSidebar'
import { useGitWorkspace } from './features/git/useGitWorkspace'
import { useRequirementsWorkspace } from './features/requirements/useRequirementsWorkspace'
import type { RequirementPhase } from './features/requirements/requirementsApi'
import { LeftPanel } from './components/LeftPanel'
import { useEmployeeTasks } from './features/employee-tasks/useEmployeeTasks'
import { WorkArea } from './components/WorkArea'
import { WorkRulesSettings } from './components/WorkRulesSettings'

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
type ToolFieldSchema = {
  key: string
  label: string
  field_type: 'text' | 'number' | 'boolean' | 'select' | 'password'
  required: boolean
  options: string[]
  placeholder?: string
}
type ToolCatalogItem = {
  kind: ToolKind
  display_name: string
  schema: { title: string; fields: ToolFieldSchema[] }
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
  const [settingsSection, setSettingsSection] = React.useState<SettingsSection>('tools')
  const [toolCatalog, setToolCatalog] = React.useState<ToolCatalogItem[]>([])
  const [toolInstances, setToolInstances] = React.useState<ToolInstance[]>([])
  const [toolKindDraft, setToolKindDraft] = React.useState<ToolKind>('claude_code')
  const [activeToolId, setActiveToolId] = React.useState<string | null>(null)
  const [toolNameDraft, setToolNameDraft] = React.useState('')
  const [toolEnabledDraft, setToolEnabledDraft] = React.useState(true)
  const [toolConfigDraft, setToolConfigDraft] = React.useState<Record<string, unknown>>({})
  const [toolSaving, setToolSaving] = React.useState(false)
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
  const requirements = useRequirementsWorkspace(API_BASE, locale, Boolean(workspace?.configured), refreshTick)
  const employeeTasks = useEmployeeTasks(
    API_BASE,
    locale,
    Boolean(workspace?.configured),
    selectedEmployeeId,
    refreshTick,
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

  const createTool = async () => {
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
  }

  const activeTool = toolInstances.find((item) => item.id === activeToolId) ?? null
  const activeCatalog =
    toolCatalog.find((item) => item.kind === activeTool?.kind) ??
    toolCatalog.find((item) => item.kind === toolKindDraft) ??
    null

  const saveActiveTool = async () => {
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
  }

  const addDepartment = () => {
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
  }

  const addRole = () => {
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
  }

  const addEmployee = async () => {
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
  }

  const hireEmployee = async () => {
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
  }

  const fireEmployee = async (id: string) => {
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
  }

  const reinstateEmployee = async (id: string) => {
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
  }

  const handoverEmployee = async (id: string) => {
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
  }

  const hardDeleteEmployee = async (id: string) => {
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
  }

  const handleNavMenuClick = (menu: NavMenu) => {
    setActiveNav(menu)
    setSettingsOpen(false)
    setWorkspace(null)
    setStatus('checking...')
    setWorkspaceError('')
    setEmployeeCreateError('')
    setEmployeeDirectory([])
    setSelectedEmployeeId(null)
    setMessageDraft('')
    setRefreshTick((prev) => prev + 1)
  }

  const startResizePanel = (event: React.MouseEvent<HTMLDivElement>) => {
    resizeStartXRef.current = event.clientX
    resizeStartWidthRef.current = sidePanelWidth
    setResizingPanel(true)
  }

  const renderSettingsCards = () => {
    if (settingsSection === 'tools') {
      return (
        <>
          <section className="settings-card">
            <div className="settings-toolbar">
              <h3 className="settings-card__title">{tt('ui.settings.tools.registration')}</h3>
              <div className="settings-toolbar__actions">
                <select
                  className="settings-input"
                  value={toolKindDraft}
                  onChange={(event) => setToolKindDraft(event.target.value as ToolKind)}
                >
                  {toolCatalog.map((tool) => (
                    <option key={tool.kind} value={tool.kind}>
                      {tool.display_name}
                    </option>
                  ))}
                </select>
                <button className="action-btn" onClick={createTool}>{tt('ui.settings.tools.add')}</button>
              </div>
            </div>
            {toolError ? <p className="workspace-setup__error">{toolError}</p> : null}
          </section>
          <section className="settings-card">
            <h3 className="settings-card__title">{tt('ui.settings.tools.list')}</h3>
            {toolInstances.length === 0 ? <p className="settings-empty">{tt('ui.settings.tools.empty')}</p> : (
              <div className="settings-list">
                {toolInstances.map((item) => (
                  <div
                    key={item.id}
                    className={`settings-list__row ${item.id === activeToolId ? 'settings-list__row--active' : ''}`}
                    onClick={() => {
                      setActiveToolId(item.id)
                      setToolNameDraft(item.name)
                      setToolEnabledDraft(item.enabled)
                      setToolConfigDraft(item.config ?? {})
                    }}
                  >
                    <div>
                      <div>{item.name}</div>
                      <div className="settings-subtext">{item.kind}</div>
                    </div>
                    <span>{item.enabled ? tt('ui.settings.tools.on') : tt('ui.settings.tools.off')}</span>
                  </div>
                ))}
              </div>
            )}
          </section>
          {activeCatalog && activeTool ? (
            <section className="settings-card">
              <h3 className="settings-card__title">{activeCatalog.schema.title}</h3>
              <div className="settings-grid">
                <input
                  className="settings-input"
                  value={toolNameDraft}
                  placeholder={tt('ui.settings.tools.name')}
                  onChange={(event) => setToolNameDraft(event.target.value)}
                />
                <label className="settings-checkbox">
                  <input
                    type="checkbox"
                    checked={toolEnabledDraft}
                    onChange={(event) => setToolEnabledDraft(event.target.checked)}
                  />
                  {tt('ui.settings.tools.enabled')}
                </label>
                <div />
                {activeCatalog.schema.fields.map((field) => (
                  <React.Fragment key={field.key}>
                    {field.field_type === 'boolean' ? (
                      <label className="settings-checkbox">
                        <input
                          type="checkbox"
                          checked={Boolean(toolConfigDraft[field.key])}
                          onChange={(event) =>
                            setToolConfigDraft((prev) => ({ ...prev, [field.key]: event.target.checked }))
                          }
                        />
                        {field.label}
                      </label>
                    ) : field.field_type === 'select' ? (
                      <select
                        className="settings-input"
                        value={String(toolConfigDraft[field.key] ?? '')}
                        onChange={(event) =>
                          setToolConfigDraft((prev) => ({ ...prev, [field.key]: event.target.value }))
                        }
                      >
                        {field.options.map((opt) => (
                          <option key={opt} value={opt}>
                            {opt}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <input
                        className="settings-input"
                        type={field.field_type === 'password' ? 'password' : 'text'}
                        value={String(toolConfigDraft[field.key] ?? '')}
                        placeholder={field.placeholder ?? field.label}
                        onChange={(event) =>
                          setToolConfigDraft((prev) => ({ ...prev, [field.key]: event.target.value }))
                        }
                      />
                    )}
                    <div className="settings-subtext">{field.label}</div>
                    <div />
                  </React.Fragment>
                ))}
              </div>
              <button className="action-btn" onClick={saveActiveTool} disabled={toolSaving}>
                {toolSaving ? tt('ui.actions.saving') : tt('ui.settings.tools.save')}
              </button>
            </section>
          ) : null}
        </>
      )
    }

    if (settingsSection === 'departments') {
      return (
        <>
          <section className="settings-card">
            <h3 className="settings-card__title">{tt('ui.settings.departments.setup')}</h3>
            <div className="settings-grid">
              <input
                className="settings-input"
                placeholder={tt('ui.settings.departments.name')}
                value={departmentForm.name}
                onChange={(event) => setDepartmentForm((prev) => ({ ...prev, name: event.target.value }))}
              />
              <input
                className="settings-input"
                placeholder={tt('ui.settings.departments.lead')}
                value={departmentForm.lead}
                onChange={(event) => setDepartmentForm((prev) => ({ ...prev, lead: event.target.value }))}
              />
              <button className="action-btn" onClick={addDepartment}>{tt('ui.settings.departments.add')}</button>
            </div>
          </section>
          <section className="settings-card">
            <h3 className="settings-card__title">{tt('ui.settings.departments.list')}</h3>
            {departments.length === 0 ? <p className="settings-empty">{tt('ui.settings.departments.empty')}</p> : (
              <div className="settings-list">
                {departments.map((item) => (
                  <div key={item.id} className="settings-list__row">
                    <div>{item.name}</div>
                    <div className="settings-subtext">{tt('ui.settings.departments.leadPrefix')}: {item.lead}</div>
                  </div>
                ))}
              </div>
            )}
          </section>
        </>
      )
    }

    if (settingsSection === 'roles') {
      return (
        <>
          <section className="settings-card">
            <h3 className="settings-card__title">{tt('ui.settings.roles.setup')}</h3>
            <div className="settings-grid">
              <input
                className="settings-input"
                placeholder={tt('ui.settings.roles.name')}
                value={roleName}
                onChange={(event) => setRoleName(event.target.value)}
              />
              <select
                className="settings-input"
                value={roleForm}
                onChange={(event) => setRoleForm(event.target.value as RoleItem['level'])}
              >
                <option value="junior">{tt('ui.settings.roles.junior')}</option>
                <option value="mid">{tt('ui.settings.roles.mid')}</option>
                <option value="senior">{tt('ui.settings.roles.senior')}</option>
              </select>
              <button className="action-btn" onClick={addRole}>{tt('ui.settings.roles.add')}</button>
            </div>
          </section>
          <section className="settings-card">
            <h3 className="settings-card__title">{tt('ui.settings.roles.list')}</h3>
            {roles.length === 0 ? <p className="settings-empty">{tt('ui.settings.roles.empty')}</p> : (
              <div className="settings-list">
                {roles.map((item) => (
                  <div key={item.id} className="settings-list__row">
                    <div>{item.name}</div>
                    <div className="settings-subtext">{item.level}</div>
                  </div>
                ))}
              </div>
            )}
          </section>
        </>
      )
    }

    if (settingsSection === 'work_rules') {
      return <WorkRulesSettings apiBase={API_BASE} locale={locale} t={tt} />
    }

    if (settingsSection === 'language') {
      return (
        <>
          <section className="settings-card">
            <h3 className="settings-card__title">{tt('ui.settings.language.title')}</h3>
            <div className="settings-grid">
              <label className="settings-subtext">{tt('ui.language.label')}</label>
              <select
                className="settings-input"
                value={locale}
                onChange={(event) => setLocale(event.target.value as Locale)}
              >
                <option value="en">{tt('ui.language.en')}</option>
                <option value="zh">{tt('ui.language.zh')}</option>
                <option value="ja">{tt('ui.language.ja')}</option>
              </select>
              <div />
            </div>
          </section>
          <section className="settings-card">
            <h3 className="settings-card__title">{tt('ui.settings.chatIdentity.title')}</h3>
            <div className="settings-grid">
              <label className="settings-subtext" htmlFor="chat-identity-display-name">
                {tt('ui.settings.chatIdentity.displayName')}
              </label>
              <input
                id="chat-identity-display-name"
                className="settings-input"
                value={chatIdentityDraft.displayName}
                onChange={(event) =>
                  setChatIdentityDraft((prev) => ({ ...prev, displayName: event.target.value }))
                }
                placeholder={tt('ui.settings.chatIdentity.displayNamePlaceholder')}
              />
              <div />
              <label className="settings-subtext" htmlFor="chat-identity-avatar-url">
                {tt('ui.settings.chatIdentity.avatarUrl')}
              </label>
              <input
                id="chat-identity-avatar-url"
                className="settings-input"
                type="url"
                inputMode="url"
                value={chatIdentityDraft.avatarUrl}
                onChange={(event) =>
                  setChatIdentityDraft((prev) => ({ ...prev, avatarUrl: event.target.value }))
                }
                placeholder={tt('ui.settings.chatIdentity.avatarUrlPlaceholder')}
              />
              <div />
              <p className="settings-subtext settings-subtext--block">{tt('ui.settings.chatIdentity.avatarHint')}</p>
            </div>
          </section>
        </>
      )
    }

    return (
      <>
        <section className="settings-card">
          <h3 className="settings-card__title">{tt('ui.settings.employees.setup')}</h3>
          <div className="settings-grid">
            <input
              className="settings-input"
              placeholder={tt('ui.settings.employees.name')}
              value={employeeForm.name}
              onChange={(event) => setEmployeeForm((prev) => ({ ...prev, name: event.target.value }))}
            />
            <select
              className="settings-input"
              value={employeeForm.department}
              onChange={(event) => setEmployeeForm((prev) => ({ ...prev, department: event.target.value }))}
            >
              <option value="">{tt('ui.settings.employees.selectDepartment')}</option>
              {departments.map((item) => (
                <option key={item.id} value={item.name}>{item.name}</option>
              ))}
            </select>
            <select
              className="settings-input"
              value={employeeForm.role}
              onChange={(event) => setEmployeeForm((prev) => ({ ...prev, role: event.target.value }))}
            >
              <option value="">{tt('ui.settings.employees.selectRole')}</option>
              {roles.map((item) => (
                <option key={item.id} value={item.name}>{item.name}</option>
              ))}
            </select>
            <button
              className="action-btn"
              onClick={() => void addEmployee()}
              disabled={creatingEmployee}
            >
              {creatingEmployee ? tt('ui.employeeList.creating') : tt('ui.settings.employees.add')}
            </button>
          </div>
        </section>
        <section className="settings-card">
          <h3 className="settings-card__title">{tt('ui.settings.employees.list')}</h3>
          {employeeDirectory.length === 0 ? <p className="settings-empty">{tt('ui.settings.employees.empty')}</p> : (
            <div className="settings-list">
              {employeeDirectory.map((item) => (
                <div key={item.id} className="settings-list__row">
                  <div>{item.name}</div>
                  <div className="settings-subtext">{item.department} / {item.role}</div>
                </div>
              ))}
            </div>
          )}
          {employeeCreateError ? <p className="workspace-setup__error">{employeeCreateError}</p> : null}
        </section>
      </>
    )
  }

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
        t={tt}
        onMenuClick={handleNavMenuClick}
        onSettingsClick={() => setSettingsOpen(true)}
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
            onCreateEmployee={() => void hireEmployee()}
            showArchived={showArchived}
            onToggleArchived={() => setShowArchived((v) => !v)}
            archivedEmployees={archivedEmployees}
            reinstateEmployeeId={reinstateEmployeeId}
            onReinstateEmployee={(id) => void reinstateEmployee(id)}
            handoverEmployeeId={handoverEmployeeId}
            onHandoverEmployee={(id) => void handoverEmployee(id)}
            hardDeletingEmployeeId={hardDeletingEmployeeId}
            onHardDeleteEmployee={(id) => void hardDeleteEmployee(id)}
            onResizeMouseDown={startResizePanel}
            gitRepos={git.repos}
            selectedGitRepoId={git.selectedRepoId}
            onSelectGitRepo={(id) => void git.selectRepo(id)}
            newGitRepoName={newGitRepoName}
            onNewGitRepoNameChange={setNewGitRepoName}
            onCreateGitRepo={() => {
              const name = newGitRepoName.trim()
              if (!name) return
              void git.createRepo(name).then(() => setNewGitRepoName(''))
            }}
            gitBusy={git.busy}
            gitError={git.error}
            gitLoading={git.loading}
            requirements={requirements.items}
            selectedRequirementId={requirements.selectedId}
            onSelectRequirement={(id) => void requirements.selectRequirement(id)}
            newRequirementTitle={newRequirementTitle}
            onNewRequirementTitleChange={setNewRequirementTitle}
            onCreateRequirement={() => {
              const title = newRequirementTitle.trim()
              if (!title) return
              void requirements.createRequirement(title).then(() => setNewRequirementTitle(''))
            }}
            requirementsBusy={requirements.busy}
            requirementsError={requirements.error}
            requirementsLoading={requirements.loading}
            requirementPhaseLabel={requirementPhaseLabel}
            showRequirementsArchived={requirements.showArchived}
            onToggleRequirementsArchived={() => requirements.setShowArchived((v: boolean) => !v)}
            archivedRequirements={requirements.archivedItems}
            onAbandonRequirement={(id) => void requirements.abandonRequirement(id)}
            onReinstateRequirement={(id) => void requirements.reinstateRequirement(id)}
            onHardDeleteRequirement={(id) => void requirements.hardDeleteRequirement(id)}
            abandoningRequirementId={requirements.abandoningId}
            reinstatingRequirementId={requirements.reinstatingId}
            hardDeletingRequirementId={requirements.hardDeletingId}
            employeeTasks={employeeTasks.tasks}
            employeeTasksLoading={employeeTasks.loading}
            employeeTasksError={employeeTasks.error}
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
        onEmployeeTasksRefresh={() => void employeeTasks.refresh()}
        t={tt}
        onOpenSettings={() => setSettingsOpen(true)}
        onSetWorkspaceInput={setWorkspaceInput}
        onSaveWorkspace={() => void saveWorkspace()}
        onMessageDraftChange={setMessageDraft}
        onCloseSettings={() => setSettingsOpen(false)}
        onSetSettingsSection={setSettingsSection}
        renderSettingsCards={renderSettingsCards}
      />
    </div>
  )
}
