import React from 'react'
import { ConfirmDialog } from './ConfirmDialog'

export type EmployeeDirectoryRecord = {
  id: string
  name: string
  department: string
  role: string
  memory_file: string
}

type EmployeeListProps = {
  employees: EmployeeDirectoryRecord[]
  selectedEmployeeId: string | null
  onSelectEmployee: (id: string) => void
  onDeleteEmployee: (id: string) => void
  deletingEmployeeId: string | null
  t: (key: string) => string
}

export function EmployeeList({
  employees,
  selectedEmployeeId,
  onSelectEmployee,
  onDeleteEmployee,
  deletingEmployeeId,
  t,
}: EmployeeListProps) {
  const [openMenuId, setOpenMenuId] = React.useState<string | null>(null)
  const [deleteConfirmId, setDeleteConfirmId] = React.useState<string | null>(null)
  const deleteConfirmName = deleteConfirmId
    ? employees.find((e) => e.id === deleteConfirmId)?.name ?? ''
    : ''

  return (
    <div className="employee-list" role="listbox" aria-label={t('ui.employeeList.title')}>
      {employees.length === 0 ? (
        <div className="employee-list__empty">{t('ui.employeeList.empty')}</div>
      ) : (
        employees.map((item, index) => {
          const isActive = item.id === selectedEmployeeId
          const unread = index === 0
          const snippet = t('ui.employeeList.snippet').replace('{name}', item.name)
          const recentTime = unread ? '09:24' : t('ui.employeeList.yesterday')
          const isDeleting = deletingEmployeeId === item.id
          const menuOpen = openMenuId === item.id
          return (
            <div
              key={item.id}
              className={`employee-item ${isActive ? 'employee-item--active' : ''}`}
            >
              <button
                type="button"
                className="employee-item__body"
                onClick={() => onSelectEmployee(item.id)}
                disabled={isDeleting}
              >
                <div className="employee-item__avatar">{item.name.slice(0, 1).toUpperCase()}</div>
                <div className="employee-item__main">
                  <div className="employee-item__name">{item.name}</div>
                  <div className="employee-item__snippet">{snippet}</div>
                </div>
                <div className="employee-item__meta">
                  <span className="employee-item__time">{recentTime}</span>
                  <span
                    className={`employee-item__dot ${unread ? 'employee-item__dot--unread' : ''}`}
                    aria-label={unread ? t('ui.employeeList.unread') : t('ui.employeeList.read')}
                  />
                </div>
              </button>
              <div className="employee-item__menu">
                <button
                  type="button"
                  className="employee-item__menu-btn"
                  title="Menu"
                  onClick={(e) => {
                    e.stopPropagation()
                    setOpenMenuId(menuOpen ? null : item.id)
                  }}
                  disabled={isDeleting}
                >
                  <i className="iconfont icon-filmetomore"></i>
                </button>
                {menuOpen && (
                  <div className="employee-item__menu-dropdown">
                    <button
                      type="button"
                      className="employee-item__menu-dropdown-item employee-item__menu-dropdown-item--delete"
                      onClick={(e) => {
                        e.stopPropagation()
                        setOpenMenuId(null)
                        setDeleteConfirmId(item.id)
                      }}
                      disabled={isDeleting}
                    >
                      {t('ui.employeeList.delete')}
                    </button>
                  </div>
                )}
              </div>
            </div>
          )
        })
      )}

      <ConfirmDialog
        open={deleteConfirmId !== null}
        title={t('ui.employeeList.deleteConfirm')}
        description={t('ui.employeeList.deleteEmployeeName').replace('{name}', deleteConfirmName)}
        confirmLabel={t('ui.employeeList.confirmDelete')}
        cancelLabel={t('ui.employeeList.cancelDelete')}
        onConfirm={() => {
          if (deleteConfirmId) onDeleteEmployee(deleteConfirmId)
          setDeleteConfirmId(null)
        }}
        onCancel={() => setDeleteConfirmId(null)}
        loading={deleteConfirmId !== null && deletingEmployeeId === deleteConfirmId}
      />
    </div>
  )
}
