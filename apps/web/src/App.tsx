import React from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Locale, resolveLocale, t } from './i18n'
import { MacWindowControls } from './components/MacWindowControls'

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
  const [departments, setDepartments] = React.useState<DepartmentItem[]>([])
  const [roles, setRoles] = React.useState<RoleItem[]>([])
  const [employees, setEmployees] = React.useState<EmployeeItem[]>([])
  // 检测是否在 Tauri 环境中运行
  const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
  // 只在 Tauri 环境中获取窗口实例
  const appWindow = isTauri ? getCurrentWindow() : null
  const tt = React.useCallback((key: string) => t(locale, key), [locale])

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
  }, [])

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

  const needsWorkspaceSetup = workspace?.configured === false
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

  const handleDragStart = async () => {
    if (!appWindow) return
    try {
      await appWindow.startDragging()
    } catch (e) {
      // Ignore drag errors (e.g., when not in Tauri environment)
    }
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
      <aside className="side-panel" onMouseDown={() => void handleDragStart()}>
        <div className="side-panel__brand">{tt('ui.brand')}</div>
        <nav className="side-panel__nav">
          <button className="nav-item nav-item--active">{tt('ui.nav.workspace')}</button>
          <button className="nav-item">{tt('ui.nav.explorer')}</button>
          <button className="nav-item">{tt('ui.nav.search')}</button>
          <button className="nav-item">{tt('ui.nav.settings')}</button>
        </nav>
        <div className="side-panel__footer">
          <span>{tt('ui.backend')}</span>
          <span className={`status status--${status}`}>{status}</span>
        </div>
      </aside>

      <section className="work-area">
        <header className="work-area__topbar" onMouseDown={() => void handleDragStart()}>
          <div className="topbar__drag" data-tauri-drag-region />
          <div className="topbar__title">
            {workspace?.configured ? tt('ui.workspace.currentProject') : tt('ui.workspace.setupTitle')}
          </div>
          <div className="topbar__actions" onMouseDown={(e) => e.stopPropagation()}>
            <label className="lang-switch">
              <span>{tt('ui.language.label')}</span>
              <select
                className="settings-input lang-switch__select"
                value={locale}
                onChange={(event) => setLocale(event.target.value as Locale)}
              >
                <option value="en">{tt('ui.language.en')}</option>
                <option value="zh">{tt('ui.language.zh')}</option>
                <option value="ja">{tt('ui.language.ja')}</option>
              </select>
            </label>
            <button className="action-btn">{tt('ui.actions.run')}</button>
            <button className="action-btn">{tt('ui.actions.share')}</button>
            <button className="action-btn" onClick={() => setSettingsOpen(true)}>{tt('ui.actions.settings')}</button>
          </div>
        </header>

        <main className="work-area__content">
          {needsWorkspaceSetup ? (
            <div className="workspace-setup">
              <h2 className="workspace-setup__title">{tt('ui.workspace.configureTitle')}</h2>
              <p className="workspace-setup__hint">
                {tt('ui.workspace.configureHint')}
              </p>
              <label className="workspace-setup__label" htmlFor="workspace-path">
                {tt('ui.workspace.pathLabel')}
              </label>
              <input
                id="workspace-path"
                className="workspace-setup__input"
                value={workspaceInput}
                onChange={(event) => setWorkspaceInput(event.target.value)}
                placeholder={tt('ui.workspace.placeholder')}
              />
              <button
                className="action-btn workspace-setup__save"
                onClick={saveWorkspace}
                disabled={savingWorkspace}
              >
                {savingWorkspace ? tt('ui.actions.saving') : tt('ui.actions.saveWorkspace')}
              </button>
              {workspaceError ? (
                <p className="workspace-setup__error">{workspaceError}</p>
              ) : null}
            </div>
          ) : (
            <div className="content-placeholder">
              <div>
                  <div>{tt('ui.workspace.panelTitle')}</div>
                  <div className="workspace-path">{workspace?.path ?? tt('ui.workspace.notConfigured')}</div>
              </div>
            </div>
          )}
        </main>
      </section>
      {settingsOpen ? (
        <div className="settings-modal">
          <div className="settings-panel">
            <aside className="settings-sidebar">
              <div className="settings-sidebar__title">{tt('ui.settings.title')}</div>
              <button
                className={`settings-nav ${settingsSection === 'tools' ? 'settings-nav--active' : ''}`}
                onClick={() => setSettingsSection('tools')}
              >
                {tt('ui.settings.menus.tools')}
              </button>
              <button
                className={`settings-nav ${settingsSection === 'departments' ? 'settings-nav--active' : ''}`}
                onClick={() => setSettingsSection('departments')}
              >
                {tt('ui.settings.menus.departments')}
              </button>
              <button
                className={`settings-nav ${settingsSection === 'roles' ? 'settings-nav--active' : ''}`}
                onClick={() => setSettingsSection('roles')}
              >
                {tt('ui.settings.menus.roles')}
              </button>
              <button
                className={`settings-nav ${settingsSection === 'employees' ? 'settings-nav--active' : ''}`}
                onClick={() => setSettingsSection('employees')}
              >
                {tt('ui.settings.menus.employees')}
              </button>
              <button className="action-btn settings-close" onClick={() => setSettingsOpen(false)}>
                {tt('ui.actions.done')}
              </button>
            </aside>
            <section className="settings-content">
              {renderSettingsCards()}
            </section>
          </div>
        </div>
      ) : null}
    </div>
  )
}
