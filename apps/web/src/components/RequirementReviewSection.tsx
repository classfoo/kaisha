import React from 'react'
import type { RequirementReview, ReviewConclusion } from '../features/requirements/requirementsApi'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import { RequirementReviewOpinionRow } from './RequirementReviewOpinionRow'

type RequirementReviewSectionProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  t: (key: string) => string
}

function conclusionLabel(t: (key: string) => string, c: ReviewConclusion | null | undefined) {
  if (c === 'adopt') return t('ui.requirements.review.conclusionAdopt')
  if (c === 'supplement') return t('ui.requirements.review.conclusionSupplement')
  return t('ui.requirements.review.conclusionPending')
}

function overallLabel(t: (key: string) => string, review: RequirementReview) {
  if (review.overall_passed) return t('ui.requirements.review.overallPassed')
  if (review.status === 'completed') return t('ui.requirements.review.overallFailed')
  return t('ui.requirements.review.overallPending')
}

export function RequirementReviewSection({ requirements, t }: RequirementReviewSectionProps) {
  const {
    review,
    reviewLoading,
    reviewRunning,
    reviewForcePassing,
    selectedId,
    forcePassReview,
  } = requirements
  const [confirmForceOpen, setConfirmForceOpen] = React.useState(false)
  const [forceError, setForceError] = React.useState('')

  const canForcePass = Boolean(selectedId) && !review?.overall_passed
  const forcePassBusy = reviewForcePassing

  const forcePassActions = canForcePass ? (
    <div className="requirement-review__actions">
      <button
        type="button"
        className="action-btn"
        disabled={forcePassBusy || reviewRunning}
        onClick={() => {
          setForceError('')
          setConfirmForceOpen(true)
        }}
      >
        {forcePassBusy ? t('ui.requirements.review.forcePassing') : t('ui.requirements.review.forcePass')}
      </button>
      {confirmForceOpen ? (
        <div className="requirement-review-confirm" role="dialog" aria-modal="true">
          <p className="requirement-review-confirm__text">{t('ui.requirements.review.forcePassConfirmText')}</p>
          <div className="requirement-review-confirm__actions">
            <button
              type="button"
              className="action-btn"
              onClick={() => {
                setConfirmForceOpen(false)
                setForceError('')
              }}
            >
              {t('ui.requirements.review.cancel')}
            </button>
            <button
              type="button"
              className="action-btn"
              onClick={() => {
                if (!selectedId) return
                setForceError('')
                void forcePassReview(selectedId)
                  .then(() => setConfirmForceOpen(false))
                  .catch((e) => setForceError(e instanceof Error ? e.message : String(e)))
              }}
              disabled={forcePassBusy}
            >
              {t('ui.requirements.review.forcePassConfirm')}
            </button>
          </div>
          {forceError ? <p className="workspace-setup__error">{forceError}</p> : null}
        </div>
      ) : null}
    </div>
  ) : null

  if (reviewLoading && !review) {
    return (
      <div className="requirement-review">
        <p className="settings-subtext">{t('ui.requirements.review.loading')}</p>
        {forcePassActions}
      </div>
    )
  }

  if (!review) {
    return (
      <div className="requirement-review">
        <p className="settings-subtext">{t('ui.requirements.review.notStarted')}</p>
        {forcePassActions}
      </div>
    )
  }

  const activeCount = review.opinions.filter(
    (op) => op.status === 'in_progress' || op.status === 'revising',
  ).length
  const tallyText = t('ui.requirements.review.tally')
    .replace('{passed}', String(review.passed_count))
    .replace('{failed}', String(review.failed_count))
    .replace('{pending}', String(review.pending_count))

  return (
    <div className="requirement-review">
      {forcePassActions}
      <div className="requirement-review__tally" role="status">
        <span className="requirement-review__tally-stat requirement-review__tally-stat--passed">
          {t('ui.requirements.review.tallyPassed')}: <strong>{review.passed_count}</strong>
        </span>
        <span className="requirement-review__tally-stat requirement-review__tally-stat--failed">
          {t('ui.requirements.review.tallyFailed')}: <strong>{review.failed_count}</strong>
        </span>
        <span className="requirement-review__tally-stat requirement-review__tally-stat--pending">
          {t('ui.requirements.review.tallyPending')}: <strong>{review.pending_count}</strong>
        </span>
        {review.undecided_count > 0 ? (
          <span className="requirement-review__tally-stat">
            {t('ui.requirements.review.tallyUndecided')}: <strong>{review.undecided_count}</strong>
          </span>
        ) : null}
        <p className="requirement-review__tally-summary settings-subtext">{tallyText}</p>
      </div>
      <div className="requirement-review__status">
        <span className="settings-subtext">{t('ui.requirements.review.statusLabel')}</span>
        <strong>
          {review.status === 'completed'
            ? t('ui.requirements.review.statusCompleted')
            : t('ui.requirements.review.statusInProgress')}
        </strong>
        <span className="settings-subtext">{t('ui.requirements.review.overallLabel')}</span>
        <strong
          className={
            review.overall_passed
              ? 'requirement-review__overall requirement-review__overall--pass'
              : review.status === 'completed'
                ? 'requirement-review__overall requirement-review__overall--fail'
                : 'requirement-review__overall'
          }
        >
          {overallLabel(t, review)}
        </strong>
        {review.status === 'completed' ? (
          <>
            <span className="settings-subtext">{t('ui.requirements.review.conclusionLabel')}</span>
            <strong>{conclusionLabel(t, review.conclusion)}</strong>
          </>
        ) : null}
      </div>
      {reviewRunning || activeCount > 0 ? (
        <p className="requirement-review__running-hint settings-subtext">
          {reviewRunning
            ? t('ui.requirements.review.running')
            : t('ui.requirements.review.activeReviewers').replace('{count}', String(activeCount))}
        </p>
      ) : null}
      <div className="requirement-review__opinions">
        <h4 className="requirement-detail__label">{t('ui.requirements.review.opinionsTitle')}</h4>
        {review.opinions.length > 0 ? (
          <ul className="requirement-review__opinion-list">
            {review.opinions.map((op) => (
              <li key={op.employee_id}>
                <RequirementReviewOpinionRow opinion={op} requirements={requirements} t={t} />
              </li>
            ))}
          </ul>
        ) : (
          <p className="settings-subtext">{t('ui.requirements.review.opinionsEmpty')}</p>
        )}
      </div>
      {review.summary ? (
        <div className="requirement-review__summary">
          <h4 className="requirement-detail__label">{t('ui.requirements.review.summaryTitle')}</h4>
          <pre className="requirement-review__summary-body">{review.summary}</pre>
        </div>
      ) : null}
    </div>
  )
}
