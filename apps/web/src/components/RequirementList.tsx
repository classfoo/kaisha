import React from 'react'
import type { RequirementPhase, RequirementSummary } from '../features/requirements/requirementsApi'

type RequirementListProps = {
  items: RequirementSummary[]
  selectedId: string | null
  onSelect: (id: string) => void
  phaseLabel: (phase: RequirementPhase) => string
  t: (key: string) => string
}

export function RequirementList({
  items,
  selectedId,
  onSelect,
  phaseLabel,
  t,
}: RequirementListProps) {
  return (
    <div className="employee-list requirement-list" role="listbox" aria-label={t('ui.requirements.listTitle')}>
      {items.length === 0 ? (
        <div className="employee-list__empty">{t('ui.requirements.empty')}</div>
      ) : (
        items.map((item) => {
          const isActive = item.id === selectedId
          return (
            <button
              key={item.id}
              type="button"
              className={`employee-item requirement-item ${isActive ? 'employee-item--active' : ''}`}
              onClick={() => onSelect(item.id)}
            >
              <div className="employee-item__avatar requirement-item__phase">
                {phaseLabel(item.phase).slice(0, 1)}
              </div>
              <div className="employee-item__main">
                <div className="employee-item__name">{item.title}</div>
                <div className="employee-item__snippet requirement-item__phase-label">
                  {phaseLabel(item.phase)}
                </div>
              </div>
            </button>
          )
        })
      )}
    </div>
  )
}
