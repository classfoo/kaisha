import React from 'react'
import { Locale, resolveLocale, t } from './i18n'
import { MacWindowControls } from './components/MacWindowControls'
import { EmployeeDirectoryRecord } from './components/EmployeeList'
import { LeftSidebar, NavMenu } from './components/LeftSidebar'
import { LeftPanel } from './components/LeftPanel'
import { WorkArea } from './components/WorkArea'

const API_BASE = import.meta.env.VITE_API_BASE ?? 'http://127.0.0.1:8080'
type WorkspaceStatus = {
  configured: boolean
  path: string | null
  source: 'env' | 'config' | 'unset'
}
type SettingsSection = 'tools' | 'departments' | 'roles' | 'employees'
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
  const [messageDraft, setMessageDraft] = React.useState('')
  const [departments, setDepartments] = React.useState<DepartmentItem[]>([])
  const [roles, setRoles] = React.useState<RoleItem[]>([])
  const [employees, setEmployees] = React.useState<EmployeeItem[]>([])
  const [creatingEmployee, setCreatingEmployee] = React.useState(false)
  const [employeeCreateError, setEmployeeCreateError] = React.useState('')
  const [activeNav, setActiveNav] = React.useState<NavMenu>('chat')
  const [refreshTick, setRefreshTick] = React.useState(0)
  const [sidePanelWidth, setSidePanelWidth] = React.useState(260)
  const [resizingPanel, setResizingPanel] = React.useState(false)
  const resizeStartXRef = React.useRef(0)
  const resizeStartWidthRef = React.useRef(260)
  const tt = React.useCallback((key: string) => t(locale, key), [locale])
  const navItems: { id: NavMenu; labelKey: string; icon: string }[] = [
    { id: 'home', labelKey: 'ui.nav.home', icon: 'H' },
    { id: 'chat', labelKey: 'ui.nav.chat', icon: 'C' },
    { id: 'build', labelKey: 'ui.nav.build', icon: 'B' },
    { id: 'test', labelKey: 'ui.nav.test', icon: 'T' },
    { id: 'produce', labelKey: 'ui.nav.produce', icon: 'P' },
  ]

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

    loadEmployees()

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

  const addEmployee = () => {
    if (!employeeForm.name.trim() || !employeeForm.department || !employeeForm.role) return
    setEmployees((prev) => [
      ...prev,
      {
        id: nextId.current++,
        name: employeeForm.name.trim(),
        department: employeeForm.department,
        role: employeeForm.role,
      },
    ])
    setEmployeeForm({ name: '', department: '', role: '' })
  }

  const createSidebarEmployee = async () => {
    if (!workspace?.configured || creatingEmployee) return
    setCreatingEmployee(true)
    setEmployeeCreateError('')
    const headers = { 'Content-Type': 'application/json', 'x-lang': locale }
    const requestBody = {
      id: `employee-${Date.now()}`,
      name: `${tt('ui.employeeList.newNamePrefix')} ${employeeDirectory.length + 1}`,
      department: 'default',
      role: 'default',
    }

    try {
      const response = await fetch(`${API_BASE}/api/employees`, {
        method: 'POST',
        headers,
        body: JSON.stringify(requestBody),
      })
      if (!response.ok) {
        const text = await response.text()
        throw new Error(text || tt('ui.employeeList.createError'))
      }

      const created: EmployeeDirectoryRecord = await response.json()
      setEmployeeDirectory((prev) => {
        const next = [...prev, created]
        next.sort((a, b) => a.id.localeCompare(b.id))
        return next
      })
      setSelectedEmployeeId(created.id)
    } catch (err) {
      setEmployeeCreateError(
        err instanceof Error && err.message ? err.message : tt('ui.employeeList.createError'),
      )
    } finally {
      setCreatingEmployee(false)
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
            <button className="action-btn" onClick={addEmployee}>{tt('ui.settings.employees.add')}</button>
          </div>
        </section>
        <section className="settings-card">
          <h3 className="settings-card__title">{tt('ui.settings.employees.list')}</h3>
          {employees.length === 0 ? <p className="settings-empty">{tt('ui.settings.employees.empty')}</p> : (
            <div className="settings-list">
              {employees.map((item) => (
                <div key={item.id} className="settings-list__row">
                  <div>{item.name}</div>
                  <div className="settings-subtext">{item.department} / {item.role}</div>
                </div>
              ))}
            </div>
          )}
        </section>
      </>
    )
  }

  return (
    <div className="app-shell">
      <MacWindowControls locale={locale} t={tt} />
      <LeftSidebar
        activeNav={activeNav}
        navItems={navItems}
        t={tt}
        onMenuClick={handleNavMenuClick}
      />
      <LeftPanel
        panelKey={`left-panel-${activeNav}-${refreshTick}`}
        sidePanelWidth={sidePanelWidth}
        employees={employeeDirectory}
        selectedEmployeeId={selectedEmployeeId}
        onSelectEmployee={setSelectedEmployeeId}
        activeNav={activeNav}
        creatingEmployee={creatingEmployee}
        employeeCreateError={employeeCreateError}
        workspaceConfigured={Boolean(workspace?.configured)}
        status={status}
        t={tt}
        onCreateEmployee={() => void createSidebarEmployee()}
        onResizeMouseDown={startResizePanel}
      />
      <WorkArea
        workAreaKey={`work-area-${activeNav}-${refreshTick}`}
        locale={locale}
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
        t={tt}
        onSetLocale={setLocale}
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
