import React from 'react'
import type { OpinionUserAction, ReviewOpinion } from '../features/requirements/requirementsApi'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'

type RequirementReviewOpinionRowProps = {
  opinion: ReviewOpinion
  requirements: ReturnType<typeof useRequirementsWorkspace>
  t: (key: string) => string
}

function statusLabel(t: (key: string) => string, status: ReviewOpinion['status']) {
  return t(`ui.requirements.review.opinionStatus.${status}`)
}

function passedLabel(t: (key: string) => string, passed: boolean | null) {
  if (passed === true) return t('ui.requirements.review.opinionPassed.yes')
  if (passed === false) return t('ui.requirements.review.opinionPassed.no')
  return t('ui.requirements.review.opinionPassed.unknown')
}

export function RequirementReviewOpinionRow({
  opinion,
  requirements,
  t,
}: RequirementReviewOpinionRowProps) {
  const {
    selectedId,
    reviewRunning,
    reviewForcePassing,
    opinionActionKey,
    opinionAction,
  } = requirements

  const [expanded, setExpanded] = React.useState(false)
  const canExpand = opinion.status === 'completed' && Boolean(opinion.content?.trim())
  const inProgress = opinion.status === 'in_progress'
  const revising = opinion.status === 'revising'
  const abandoned = opinion.status === 'abandoned'
  const failed = opinion.status === 'completed' && opinion.passed === false

  const rowBusy = opinionActionKey?.startsWith(`${opinion.employee_id}:`) ?? false
  const actionsDisabled =
    !selectedId || rowBusy || reviewRunning || reviewForcePassing || inProgress || revising

  const onAction = (action: OpinionUserAction) => {
    if (!selectedId || actionsDisabled) return
    void opinionAction(selectedId, opinion.employee_id, action).catch(() => undefined)
  }

  return (
    <article
      className={`requirement-review__opinion requirement-review__opinion--${opinion.status}`}
    >
      <button
        type="button"
        className="requirement-review__opinion-toggle"
        disabled={!canExpand}
        aria-expanded={canExpand ? expanded : undefined}
        onClick={() => {
          if (canExpand) setExpanded((v) => !v)
        }}
      >
        <span className="requirement-review__opinion-main">
          <strong className="requirement-review__opinion-name">{opinion.employee_name}</strong>
          <span className="settings-subtext requirement-review__opinion-role">{opinion.role}</span>
        </span>
        <span className="requirement-review__opinion-badges">
          <span
            className={`requirement-review__badge requirement-review__badge--status requirement-review__badge--status-${opinion.status}`}
          >
            {inProgress || revising ? (
              <span className="requirement-review__badge-pulse" aria-hidden="true" />
            ) : null}
            {statusLabel(t, opinion.status)}
          </span>
          {opinion.status === 'completed' ? (
            <span
              className={`requirement-review__badge requirement-review__badge--passed requirement-review__badge--passed-${opinion.passed === true ? 'yes' : opinion.passed === false ? 'no' : 'unknown'}`}
            >
              {passedLabel(t, opinion.passed)}
            </span>
          ) : null}
        </span>
        {canExpand ? (
          <span className="requirement-review__opinion-chevron" aria-hidden="true">
            {expanded ? '▾' : '▸'}
          </span>
        ) : null}
      </button>

      <div className="requirement-review__opinion-toolbar">
        <button
          type="button"
          className="action-btn action-btn--compact"
          disabled={actionsDisabled}
          onClick={() => onAction('rerun')}
        >
          {rowBusy && opinionActionKey === `${opinion.employee_id}:rerun`
            ? t('ui.requirements.review.opinionActions.busy')
            : t('ui.requirements.review.opinionActions.rerun')}
        </button>
        <button
          type="button"
          className="action-btn action-btn--compact"
          disabled={actionsDisabled || abandoned}
          onClick={() => onAction('pass')}
        >
          {rowBusy && opinionActionKey === `${opinion.employee_id}:pass`
            ? t('ui.requirements.review.opinionActions.busy')
            : t('ui.requirements.review.opinionActions.pass')}
        </button>
        <button
          type="button"
          className="action-btn action-btn--compact"
          disabled={actionsDisabled || abandoned}
          onClick={() => onAction('fail')}
        >
          {rowBusy && opinionActionKey === `${opinion.employee_id}:fail`
            ? t('ui.requirements.review.opinionActions.busy')
            : t('ui.requirements.review.opinionActions.fail')}
        </button>
        <button
          type="button"
          className="action-btn action-btn--compact"
          disabled={actionsDisabled || abandoned}
          onClick={() => onAction('abandon')}
        >
          {rowBusy && opinionActionKey === `${opinion.employee_id}:abandon`
            ? t('ui.requirements.review.opinionActions.busy')
            : t('ui.requirements.review.opinionActions.abandon')}
        </button>
      </div>

      {inProgress ? (
        <p className="requirement-review__opinion-hint settings-subtext">
          {t('ui.requirements.review.agentRunning')}
        </p>
      ) : null}
      {revising ? (
        <p className="requirement-review__opinion-hint settings-subtext">
          {t('ui.requirements.review.agentRevising')}
        </p>
      ) : null}
      {abandoned ? (
        <p className="requirement-review__opinion-hint settings-subtext">
          {t('ui.requirements.review.opinionAbandonedHint')}
        </p>
      ) : null}
      {failed ? (
        <p className="requirement-review__opinion-hint requirement-review__opinion-hint--fail settings-subtext">
          {t('ui.requirements.review.mustReviseHint')}
        </p>
      ) : null}
      {canExpand && expanded ? (
        <pre className="requirement-review__opinion-body">{opinion.content}</pre>
      ) : null}
    </article>
  )
}
