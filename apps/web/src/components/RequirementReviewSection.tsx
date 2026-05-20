import React from 'react'
import type { RequirementReview, ReviewConclusion } from '../features/requirements/requirementsApi'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'

type RequirementReviewSectionProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  t: (key: string) => string
}

function conclusionLabel(t: (key: string) => string, c: ReviewConclusion | null | undefined) {
  if (c === 'adopt') return t('ui.requirements.review.conclusionAdopt')
  if (c === 'supplement') return t('ui.requirements.review.conclusionSupplement')
  return t('ui.requirements.review.conclusionPending')
}

export function RequirementReviewSection({ requirements, t }: RequirementReviewSectionProps) {
  const { review, reviewLoading, reviewRunning } = requirements

  if (reviewLoading && !review) {
    return <p className="settings-subtext">{t('ui.requirements.review.loading')}</p>
  }

  if (!review) {
    return <p className="settings-subtext">{t('ui.requirements.review.notStarted')}</p>
  }

  return (
    <div className="requirement-review">
      <div className="requirement-review__status">
        <span className="settings-subtext">{t('ui.requirements.review.statusLabel')}</span>
        <strong>
          {review.status === 'completed'
            ? t('ui.requirements.review.statusCompleted')
            : t('ui.requirements.review.statusInProgress')}
        </strong>
        {review.status === 'completed' ? (
          <>
            <span className="settings-subtext">{t('ui.requirements.review.conclusionLabel')}</span>
            <strong>{conclusionLabel(t, review.conclusion)}</strong>
          </>
        ) : null}
      </div>
      {reviewRunning ? (
        <p className="settings-subtext">{t('ui.requirements.review.running')}</p>
      ) : null}
      {review.opinions.length > 0 ? (
        <div className="requirement-review__opinions">
          <h4 className="requirement-detail__label">{t('ui.requirements.review.opinionsTitle')}</h4>
          {review.opinions.map((op) => (
            <article key={op.employee_id} className="requirement-review__opinion">
              <header className="requirement-review__opinion-head">
                <strong>{op.employee_name}</strong>
                <span className="settings-subtext">{op.role}</span>
              </header>
              <pre className="requirement-review__opinion-body">{op.content}</pre>
            </article>
          ))}
        </div>
      ) : null}
      {review.summary ? (
        <div className="requirement-review__summary">
          <h4 className="requirement-detail__label">{t('ui.requirements.review.summaryTitle')}</h4>
          <pre className="requirement-review__summary-body">{review.summary}</pre>
        </div>
      ) : null}
    </div>
  )
}
