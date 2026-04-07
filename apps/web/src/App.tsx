import React from 'react'
import { Locale, resolveLocale, t } from './i18n'

const API_BASE = import.meta.env.VITE_API_BASE ?? 'http://127.0.0.1:8080'
type WorkspaceStatus = {
  configured: boolean
  path: string | null
  source: 'env' | 'config' | 'unset'
}
type SettingsSection = 'tools' | 'departments' | 'roles' | 'employees'
type ToolItem = { id: number; name: string; command: string; enabled: boolean }
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
  const [toolForm, setToolForm] = React.useState({ name: '', command: '', enabled: true })
  const [departmentForm, setDepartmentForm] = React.useState({ name: '', lead: '' })
  const [roleForm, setRoleForm] = React.useState<RoleItem['level']>('mid')
  const [roleName, setRoleName] = React.useState('')
  const [employeeForm, setEmployeeForm] = React.useState({
    name: '',
    department: '',
    role: '',
  })
  const [tools, setTools] = React.useState<ToolItem[]>([])
  const [departments, setDepartments] = React.useState<DepartmentItem[]>([])
  const [roles, setRoles] = React.useState<RoleItem[]>([])
  const [employees, setEmployees] = React.useState<EmployeeItem[]>([])
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

  const addTool = () => {
    if (!toolForm.name.trim() || !toolForm.command.trim()) return
    setTools((prev) => [
      ...prev,
      {
        id: nextId.current++,
        name: toolForm.name.trim(),
        command: toolForm.command.trim(),
        enabled: toolForm.enabled,
      },
    ])
    setToolForm({ name: '', command: '', enabled: true })
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

  const renderSettingsCards = () => {
    if (settingsSection === 'tools') {
      return (
        <>
          <section className="settings-card">
            <h3 className="settings-card__title">{tt('ui.settings.tools.registration')}</h3>
            <div className="settings-grid">
              <input
                className="settings-input"
                placeholder={tt('ui.settings.tools.name')}
                value={toolForm.name}
                onChange={(event) => setToolForm((prev) => ({ ...prev, name: event.target.value }))}
              />
              <input
                className="settings-input"
                placeholder={tt('ui.settings.tools.command')}
                value={toolForm.command}
                onChange={(event) => setToolForm((prev) => ({ ...prev, command: event.target.value }))}
              />
              <label className="settings-checkbox">
                <input
                  type="checkbox"
                  checked={toolForm.enabled}
                  onChange={(event) => setToolForm((prev) => ({ ...prev, enabled: event.target.checked }))}
                />
                {tt('ui.settings.tools.enabled')}
              </label>
              <button className="action-btn" onClick={addTool}>{tt('ui.settings.tools.add')}</button>
            </div>
          </section>
          <section className="settings-card">
            <h3 className="settings-card__title">{tt('ui.settings.tools.list')}</h3>
            {tools.length === 0 ? <p className="settings-empty">{tt('ui.settings.tools.empty')}</p> : (
              <div className="settings-list">
                {tools.map((item) => (
                  <div key={item.id} className="settings-list__row">
                    <div>
                      <div>{item.name}</div>
                      <div className="settings-subtext">{item.command}</div>
                    </div>
                    <label className="settings-switch">
                      <input
                        type="checkbox"
                        checked={item.enabled}
                        onChange={(event) =>
                          setTools((prev) =>
                            prev.map((tool) =>
                              tool.id === item.id ? { ...tool, enabled: event.target.checked } : tool,
                            ),
                          )
                        }
                      />
                      <span>{item.enabled ? tt('ui.settings.tools.on') : tt('ui.settings.tools.off')}</span>
                    </label>
                  </div>
                ))}
              </div>
            )}
          </section>
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
      <aside className="side-panel">
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
        <header className="work-area__topbar">
          <div className="topbar__title">
            {workspace?.configured ? tt('ui.workspace.currentProject') : tt('ui.workspace.setupTitle')}
          </div>
          <div className="topbar__actions">
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
