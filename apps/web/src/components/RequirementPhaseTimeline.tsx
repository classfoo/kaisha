import React from 'react'
import { REQUIREMENT_PHASES, type RequirementPhase } from '../features/requirements/requirementsApi'

type RequirementPhaseTimelineProps = {
  phase: RequirementPhase
  phaseLabel: (phase: RequirementPhase) => string
  disabled?: boolean
  onPhaseChange: (phase: RequirementPhase) => void
  t: (key: string) => string
}

function phaseIndex(phase: RequirementPhase): number {
  return REQUIREMENT_PHASES.indexOf(phase)
}

export function RequirementPhaseTimeline({
  phase,
  phaseLabel,
  disabled = false,
  onPhaseChange,
  t,
}: RequirementPhaseTimelineProps) {
  const currentIndex = phaseIndex(phase)
  const progressPct =
    REQUIREMENT_PHASES.length <= 1
      ? 0
      : (currentIndex / (REQUIREMENT_PHASES.length - 1)) * 100

  return (
    <div
      className="requirement-timeline"
      role="list"
      aria-label={t('ui.requirements.timelineLabel')}
    >
      <p className="requirement-timeline__hint">{t('ui.requirements.timelineHint')}</p>
      <div className="requirement-timeline__rail">
        <div className="requirement-timeline__line" aria-hidden="true">
          <div
            className="requirement-timeline__line-fill"
            style={{ width: `${progressPct}%` }}
          />
        </div>
        <ol className="requirement-timeline__track">
          {REQUIREMENT_PHASES.map((item, index) => {
            const isCurrent = index === currentIndex
            const isDone = index < currentIndex
            const stateClass = isCurrent ? 'current' : isDone ? 'done' : 'upcoming'
            return (
              <li
                key={item}
                className={`requirement-timeline__step requirement-timeline__step--${stateClass}`}
                role="listitem"
              >
                <button
                  type="button"
                  className="requirement-timeline__node"
                  disabled={disabled}
                  aria-current={isCurrent ? 'step' : undefined}
                  aria-label={phaseLabel(item)}
                  title={phaseLabel(item)}
                  onClick={() => onPhaseChange(item)}
                >
                  <span className="requirement-timeline__dot" aria-hidden="true" />
                </button>
                <span className="requirement-timeline__label">{phaseLabel(item)}</span>
              </li>
            )
          })}
        </ol>
      </div>
    </div>
  )
}
