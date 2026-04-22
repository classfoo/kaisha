import React from 'react'
import { EmployeeDirectoryRecord, EmployeeList } from './EmployeeList'

type LeftPanelProps = {
  panelKey: string
  sidePanelWidth: number
  employees: EmployeeDirectoryRecord[]
  selectedEmployeeId: string | null
  onSelectEmployee: (id: string) => void
  creatingEmployee: boolean
  employeeCreateError: string
  workspaceConfigured: boolean
  status: string
  t: (key: string) => string
  onCreateEmployee: () => void
  onResizeMouseDown: (event: React.MouseEvent<HTMLDivElement>) => void
}

export function LeftPanel({
  panelKey,
  sidePanelWidth,
  employees,
  selectedEmployeeId,
  onSelectEmployee,
  creatingEmployee,
  employeeCreateError,
  workspaceConfigured,
  status,
  t,
  onCreateEmployee,
  onResizeMouseDown,
}: LeftPanelProps) {
  return (
    <div className="side-panel-wrap" style={{ width: `${sidePanelWidth}px` }} key={panelKey}>
      <aside className="side-panel" data-tauri-drag-region>
        <div className="side-panel__brand">{t('ui.brand')}</div>
        <EmployeeList
          employees={employees}
          selectedEmployeeId={selectedEmployeeId}
          onSelectEmployee={onSelectEmployee}
          t={t}
        />
        <div className="side-panel__footer">
          <div className="side-panel__toolbar">
            <button
              className="action-btn side-panel__add-employee"
              onClick={onCreateEmployee}
              disabled={!workspaceConfigured || creatingEmployee}
            >
              {creatingEmployee ? t('ui.employeeList.creating') : t('ui.employeeList.create')}
            </button>
            {employeeCreateError ? (
              <div className="side-panel__error">{employeeCreateError}</div>
            ) : null}
          </div>
          <div className="side-panel__status">
            <span>{t('ui.backend')}</span>
            <span className={`status status--${status}`}>{status}</span>
          </div>
        </div>
      </aside>
      <div className="side-panel-resizer" onMouseDown={onResizeMouseDown} />
    </div>
  )
}
