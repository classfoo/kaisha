import React from 'react'
import type { Locale } from '../i18n'
import { t } from '../i18n'
import { WorkRulesSettings } from './WorkRulesSettings'
import type { EmployeeDirectoryRecord } from './EmployeeList'

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
type SettingsSection = 'tools' | 'departments' | 'roles' | 'employees' | 'work_rules' | 'language'

type ChatIdentityDraft = { displayName: string; avatarUrl: string }

type SettingsCardsProps = {
  apiBase: string
  locale: Locale
  section: SettingsSection
  tt: (key: string) => string
  // Tools
  toolKindDraft: ToolKind
  toolCatalog: ToolCatalogItem[]
  toolInstances: ToolInstance[]
  activeToolId: string | null
  toolNameDraft: string
  toolEnabledDraft: boolean
  toolConfigDraft: Record<string, unknown>
  toolError: string
  toolSaving: boolean
  onCreateTool: () => void
  onSaveTool: () => void
  onSelectToolInstance: (id: string, name: string, enabled: boolean, config: Record<string, unknown>) => void
  onToolKindDraftChange: (kind: ToolKind) => void
  onToolNameDraftChange: (name: string) => void
  onToolEnabledDraftChange: (enabled: boolean) => void
  onToolConfigDraftChange: (config: Record<string, unknown>) => void
  // Departments
  departmentForm: { name: string; lead: string }
  departments: DepartmentItem[]
  onAddDepartment: () => void
  onDepartmentFormChange: (form: { name: string; lead: string }) => void
  // Roles
  roleName: string
  roleForm: RoleItem['level']
  roles: RoleItem[]
  onAddRole: () => void
  onRoleNameChange: (name: string) => void
  onRoleFormChange: (level: RoleItem['level']) => void
  // Employees
  employeeForm: { name: string; department: string; role: string }
  employeeDirectory: EmployeeDirectoryRecord[]
  employeeCreateError: string
  creatingEmployee: boolean
  onAddEmployee: () => void
  onEmployeeFormChange: (form: { name: string; department: string; role: string }) => void
  // Language / Chat Identity
  chatIdentityDraft: ChatIdentityDraft
  onLocaleChange: (locale: Locale) => void
  onChatIdentityDraftChange: (draft: ChatIdentityDraft) => void
}

export const SettingsCards = React.memo(function SettingsCards(props: SettingsCardsProps) {
  const {
    locale,
    section,
    tt,
    toolKindDraft,
    toolCatalog,
    toolInstances,
    activeToolId,
    toolNameDraft,
    toolEnabledDraft,
    toolConfigDraft,
    toolError,
    toolSaving,
    onCreateTool,
    onSaveTool,
    onSelectToolInstance,
    onToolKindDraftChange,
    onToolNameDraftChange,
    onToolEnabledDraftChange,
    onToolConfigDraftChange,
    departmentForm,
    departments,
    onAddDepartment,
    onDepartmentFormChange,
    roleName,
    roleForm,
    roles,
    onAddRole,
    onRoleNameChange,
    onRoleFormChange,
    employeeForm,
    employeeDirectory,
    employeeCreateError,
    creatingEmployee,
    onAddEmployee,
    onEmployeeFormChange,
    chatIdentityDraft,
    onLocaleChange,
    onChatIdentityDraftChange,
  } = props

  const activeTool = toolInstances.find((item) => item.id === activeToolId) ?? null
  const activeCatalog =
    toolCatalog.find((item) => item.kind === activeTool?.kind) ??
    toolCatalog.find((item) => item.kind === toolKindDraft) ??
    null

  if (section === 'tools') {
    return (
      <>
        <section className="settings-card">
          <div className="settings-toolbar">
            <h3 className="settings-card__title">{tt('ui.settings.tools.registration')}</h3>
            <div className="settings-toolbar__actions">
              <select
                className="settings-input"
                value={toolKindDraft}
                onChange={(event) => onToolKindDraftChange(event.target.value as ToolKind)}
              >
                {toolCatalog.map((tool) => (
                  <option key={tool.kind} value={tool.kind}>
                    {tool.display_name}
                  </option>
                ))}
              </select>
              <button className="action-btn" onClick={onCreateTool}>{tt('ui.settings.tools.add')}</button>
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
                  onClick={() => onSelectToolInstance(item.id, item.name, item.enabled, item.config ?? {})}
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
                onChange={(event) => onToolNameDraftChange(event.target.value)}
              />
              <label className="settings-checkbox">
                <input
                  type="checkbox"
                  checked={toolEnabledDraft}
                  onChange={(event) => onToolEnabledDraftChange(event.target.checked)}
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
                          onToolConfigDraftChange({ ...toolConfigDraft, [field.key]: event.target.checked })
                        }
                      />
                      {field.label}
                    </label>
                  ) : field.field_type === 'select' ? (
                    <select
                      className="settings-input"
                      value={String(toolConfigDraft[field.key] ?? '')}
                      onChange={(event) =>
                        onToolConfigDraftChange({ ...toolConfigDraft, [field.key]: event.target.value })
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
                        onToolConfigDraftChange({ ...toolConfigDraft, [field.key]: event.target.value })
                      }
                    />
                  )}
                  <div className="settings-subtext">{field.label}</div>
                  <div />
                </React.Fragment>
              ))}
            </div>
            <button className="action-btn" onClick={onSaveTool} disabled={toolSaving}>
              {toolSaving ? tt('ui.actions.saving') : tt('ui.settings.tools.save')}
            </button>
          </section>
        ) : null}
      </>
    )
  }

  if (section === 'departments') {
    return (
      <>
        <section className="settings-card">
          <h3 className="settings-card__title">{tt('ui.settings.departments.setup')}</h3>
          <div className="settings-grid">
            <input
              className="settings-input"
              placeholder={tt('ui.settings.departments.name')}
              value={departmentForm.name}
              onChange={(event) => onDepartmentFormChange({ ...departmentForm, name: event.target.value })}
            />
            <input
              className="settings-input"
              placeholder={tt('ui.settings.departments.lead')}
              value={departmentForm.lead}
              onChange={(event) => onDepartmentFormChange({ ...departmentForm, lead: event.target.value })}
            />
            <button className="action-btn" onClick={onAddDepartment}>{tt('ui.settings.departments.add')}</button>
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

  if (section === 'roles') {
    return (
      <>
        <section className="settings-card">
          <h3 className="settings-card__title">{tt('ui.settings.roles.setup')}</h3>
          <div className="settings-grid">
            <input
              className="settings-input"
              placeholder={tt('ui.settings.roles.name')}
              value={roleName}
              onChange={(event) => onRoleNameChange(event.target.value)}
            />
            <select
              className="settings-input"
              value={roleForm}
              onChange={(event) => onRoleFormChange(event.target.value as RoleItem['level'])}
            >
              <option value="junior">{tt('ui.settings.roles.junior')}</option>
              <option value="mid">{tt('ui.settings.roles.mid')}</option>
              <option value="senior">{tt('ui.settings.roles.senior')}</option>
            </select>
            <button className="action-btn" onClick={onAddRole}>{tt('ui.settings.roles.add')}</button>
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

  if (section === 'work_rules') {
    return <WorkRulesSettings apiBase={props.apiBase} locale={locale} t={tt} />
  }

  if (section === 'language') {
    return (
      <>
        <section className="settings-card">
          <h3 className="settings-card__title">{tt('ui.settings.language.title')}</h3>
          <div className="settings-grid">
            <label className="settings-subtext">{tt('ui.language.label')}</label>
            <select
              className="settings-input"
              value={locale}
              onChange={(event) => onLocaleChange(event.target.value as Locale)}
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
                onChatIdentityDraftChange({ ...chatIdentityDraft, displayName: event.target.value })
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
                onChatIdentityDraftChange({ ...chatIdentityDraft, avatarUrl: event.target.value })
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
            onChange={(event) => onEmployeeFormChange({ ...employeeForm, name: event.target.value })}
          />
          <select
            className="settings-input"
            value={employeeForm.department}
            onChange={(event) => onEmployeeFormChange({ ...employeeForm, department: event.target.value })}
          >
            <option value="">{tt('ui.settings.employees.selectDepartment')}</option>
            {departments.map((item) => (
              <option key={item.id} value={item.name}>{item.name}</option>
            ))}
          </select>
          <select
            className="settings-input"
            value={employeeForm.role}
            onChange={(event) => onEmployeeFormChange({ ...employeeForm, role: event.target.value })}
          >
            <option value="">{tt('ui.settings.employees.selectRole')}</option>
            {roles.map((item) => (
              <option key={item.id} value={item.name}>{item.name}</option>
            ))}
          </select>
          <button
            className="action-btn"
            onClick={() => void onAddEmployee()}
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
})
