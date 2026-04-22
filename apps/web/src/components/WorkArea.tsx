import React from 'react'
import { Locale } from '../i18n'
import { EmployeeChatPanel } from './EmployeeChatPanel'
import { EmployeeDirectoryRecord } from './EmployeeList'

type WorkAreaProps = {
  workAreaKey: string
  locale: Locale
  workspaceConfigured: boolean
  workspacePath: string | null
  workspaceInput: string
  workspaceError: string
  savingWorkspace: boolean
  messageDraft: string
  settingsOpen: boolean
  settingsSection: 'tools' | 'departments' | 'roles' | 'employees'
  employees: EmployeeDirectoryRecord[]
  selectedEmployeeId: string | null
  t: (key: string) => string
  onSetLocale: (value: Locale) => void
  onOpenSettings: () => void
  onSetWorkspaceInput: (value: string) => void
  onSaveWorkspace: () => void
  onMessageDraftChange: (value: string) => void
  onCloseSettings: () => void
  onSetSettingsSection: (value: 'tools' | 'departments' | 'roles' | 'employees') => void
  renderSettingsCards: () => React.ReactNode
}

export function WorkArea({
  workAreaKey,
  locale,
  workspaceConfigured,
  workspacePath,
  workspaceInput,
  workspaceError,
  savingWorkspace,
  messageDraft,
  settingsOpen,
  settingsSection,
  employees,
  selectedEmployeeId,
  t,
  onSetLocale,
  onOpenSettings,
  onSetWorkspaceInput,
  onSaveWorkspace,
  onMessageDraftChange,
  onCloseSettings,
  onSetSettingsSection,
  renderSettingsCards,
}: WorkAreaProps) {
  const needsWorkspaceSetup = workspaceConfigured === false

  return (
    <>
      <section className="work-area" key={workAreaKey}>
        <header className="work-area__topbar" data-tauri-drag-region>
          <div className="topbar__drag" data-tauri-drag-region />
          <div className="topbar__title">
            {workspaceConfigured ? t('ui.workspace.currentProject') : t('ui.workspace.setupTitle')}
          </div>
          <div className="topbar__actions" onMouseDown={(e) => e.stopPropagation()}>
            <label className="lang-switch">
              <span>{t('ui.language.label')}</span>
              <select
                className="settings-input lang-switch__select"
                value={locale}
                onChange={(event) => onSetLocale(event.target.value as Locale)}
              >
                <option value="en">{t('ui.language.en')}</option>
                <option value="zh">{t('ui.language.zh')}</option>
                <option value="ja">{t('ui.language.ja')}</option>
              </select>
            </label>
            <button className="action-btn">{t('ui.actions.run')}</button>
            <button className="action-btn">{t('ui.actions.share')}</button>
            <button className="action-btn" onClick={onOpenSettings}>{t('ui.actions.settings')}</button>
          </div>
        </header>

        <main className="work-area__content">
          {needsWorkspaceSetup ? (
            <div className="workspace-setup">
              <h2 className="workspace-setup__title">{t('ui.workspace.configureTitle')}</h2>
              <p className="workspace-setup__hint">
                {t('ui.workspace.configureHint')}
              </p>
              <label className="workspace-setup__label" htmlFor="workspace-path">
                {t('ui.workspace.pathLabel')}
              </label>
              <input
                id="workspace-path"
                className="workspace-setup__input"
                value={workspaceInput}
                onChange={(event) => onSetWorkspaceInput(event.target.value)}
                placeholder={t('ui.workspace.placeholder')}
              />
              <button
                className="action-btn workspace-setup__save"
                onClick={onSaveWorkspace}
                disabled={savingWorkspace}
              >
                {savingWorkspace ? t('ui.actions.saving') : t('ui.actions.saveWorkspace')}
              </button>
              {workspaceError ? (
                <p className="workspace-setup__error">{workspaceError}</p>
              ) : null}
            </div>
          ) : (
            <EmployeeChatPanel
              employees={employees}
              selectedEmployeeId={selectedEmployeeId}
              messageDraft={messageDraft}
              onMessageDraftChange={onMessageDraftChange}
              workspacePath={workspacePath}
              t={t}
            />
          )}
        </main>
      </section>
      {settingsOpen ? (
        <div className="settings-modal">
          <div className="settings-panel">
            <aside className="settings-sidebar">
              <div className="settings-sidebar__title">{t('ui.settings.title')}</div>
              <button
                className={`settings-nav ${settingsSection === 'tools' ? 'settings-nav--active' : ''}`}
                onClick={() => onSetSettingsSection('tools')}
              >
                {t('ui.settings.menus.tools')}
              </button>
              <button
                className={`settings-nav ${settingsSection === 'departments' ? 'settings-nav--active' : ''}`}
                onClick={() => onSetSettingsSection('departments')}
              >
                {t('ui.settings.menus.departments')}
              </button>
              <button
                className={`settings-nav ${settingsSection === 'roles' ? 'settings-nav--active' : ''}`}
                onClick={() => onSetSettingsSection('roles')}
              >
                {t('ui.settings.menus.roles')}
              </button>
              <button
                className={`settings-nav ${settingsSection === 'employees' ? 'settings-nav--active' : ''}`}
                onClick={() => onSetSettingsSection('employees')}
              >
                {t('ui.settings.menus.employees')}
              </button>
              <button className="action-btn settings-close" onClick={onCloseSettings}>
                {t('ui.actions.done')}
              </button>
            </aside>
            <section className="settings-content">
              {renderSettingsCards()}
            </section>
          </div>
        </div>
      ) : null}
    </>
  )
}
