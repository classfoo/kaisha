import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import type { RequirementPhase } from '../features/requirements/requirementsApi'
import { RequirementPhaseTimeline } from './RequirementPhaseTimeline'
import { RequirementPhaseContent } from './RequirementPhaseContent'

type RequirementDetailPanelProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  phaseLabel: (phase: RequirementPhase) => string
  t: (key: string) => string
}

export function RequirementDetailPanel({ requirements, phaseLabel, t }: RequirementDetailPanelProps) {
  const { detail, loading, busy, error, saveRequirement, runReview, reviewRunning, reviewForcePassing, confirming, abandoning, reconfirming, confirmRequirement, abandonRequirement, reconfirmRequirement } = requirements
  const [titleDraft, setTitleDraft] = React.useState('')
  const [phaseDraft, setPhaseDraft] = React.useState<RequirementPhase>('collection')
  const [viewPhase, setViewPhase] = React.useState<RequirementPhase>('collection')
  const [contentDraft, setContentDraft] = React.useState('')
  const [dirty, setDirty] = React.useState(false)
  const [saveError, setSaveError] = React.useState('')
  const [confirmReviewOpen, setConfirmReviewOpen] = React.useState(false)
  const [reviewError, setReviewError] = React.useState('')
  const [confirmConfirmOpen, setConfirmConfirmOpen] = React.useState(false)
  const [confirmActionError, setConfirmActionError] = React.useState('')
  const [confirmAbandonOpen, setConfirmAbandonOpen] = React.useState(false)
  const [abandonActionError, setAbandonActionError] = React.useState('')
  const [confirmReconfirmOpen, setConfirmReconfirmOpen] = React.useState(false)
  const [reconfirmActionError, setReconfirmActionError] = React.useState('')
  const [confirmForceOpen, setConfirmForceOpen] = React.useState(false)
  const [forceError, setForceError] = React.useState('')
  const [createTaskOpen, setCreateTaskOpen] = React.useState(false)
  const [createTaskTitle, setCreateTaskTitle] = React.useState('')
  const [createTaskAssignee, setCreateTaskAssignee] = React.useState('')
  const [createError, setCreateError] = React.useState('')

  React.useEffect(() => {
    if (!detail) {
      setTitleDraft('')
      setPhaseDraft('collection')
      setViewPhase('collection')
      setContentDraft('')
      setDirty(false)
      return
    }
    setTitleDraft(detail.title)
    setPhaseDraft(detail.phase)
    setViewPhase(detail.phase)
    setContentDraft(detail.content)
    setDirty(false)
    setSaveError('')
  }, [detail?.id, detail?.title, detail?.phase, detail?.content])

  const markDirty = () => setDirty(true)

  const onSave = async () => {
    if (!detail) return
    setSaveError('')
    try {
      await saveRequirement({
        title: titleDraft.trim(),
        phase: phaseDraft,
        content: contentDraft,
      })
      setDirty(false)
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e))
    }
  }

  const renderPhaseToolbar = () => {
    const phase = viewPhase
    const isPhaseActive = phase === detail?.phase

    // collection phase: save button
    if (phase === 'collection') {
      return (
        <div className="requirement-phase-toolbar">
          <button
            type="button"
            className="action-btn"
            onClick={() => void onSave()}
            disabled={busy || !dirty || !isPhaseActive}
          >
            {busy ? t('ui.requirements.saving') : t('ui.requirements.save')}
          </button>
        </div>
      )
    }

    // review phase: enter review, force pass
    if (phase === 'review') {
      return (
        <div className="requirement-phase-toolbar">
          <button
            type="button"
            className="action-btn"
            onClick={() => setConfirmReviewOpen(true)}
            disabled={busy || reviewRunning || !isPhaseActive}
          >
            {reviewRunning ? t('ui.requirements.review.running') : t('ui.requirements.review.enter')}
          </button>
          {!reviewRunning && (
            <button
              type="button"
              className="action-btn"
              onClick={() => setConfirmForceOpen(true)}
              disabled={busy || reviewForcePassing || !isPhaseActive}
            >
              {reviewForcePassing ? t('ui.requirements.review.forcePassing') : t('ui.requirements.review.forcePass')}
            </button>
          )}
          {confirmReviewOpen ? (
            <div className="requirement-review-confirm" role="dialog" aria-modal="true">
              <p className="requirement-review-confirm__text">{t('ui.requirements.review.confirmText')}</p>
              <div className="requirement-review-confirm__actions">
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    setConfirmReviewOpen(false)
                    setReviewError('')
                  }}
                >
                  {t('ui.requirements.review.cancel')}
                </button>
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    if (!detail) return
                    setReviewError('')
                    void runReview(detail.id)
                      .then(() => {
                        setConfirmReviewOpen(false)
                        setViewPhase('review')
                      })
                      .catch((e) => setReviewError(e instanceof Error ? e.message : String(e)))
                  }}
                  disabled={reviewRunning}
                >
                  {t('ui.requirements.review.confirm')}
                </button>
              </div>
              {reviewError ? <p className="workspace-setup__error">{reviewError}</p> : null}
            </div>
          ) : null}
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
                    if (!detail) return
                    setForceError('')
                    void requirements.forcePassReview(detail.id)
                      .then(() => {
                        setConfirmForceOpen(false)
                      })
                      .catch((e) => setForceError(e instanceof Error ? e.message : String(e)))
                  }}
                  disabled={reviewForcePassing}
                >
                  {t('ui.requirements.review.forcePassConfirm')}
                </button>
              </div>
              {forceError ? <p className="workspace-setup__error">{forceError}</p> : null}
            </div>
          ) : null}
        </div>
      )
    }

    // confirm phase: confirm, abandon, reconfirm based on status
    if (phase === 'confirm') {
      const confirmStatus = detail?.confirm_status
      const isAbandoned = confirmStatus === 'abandoned'
      const isConfirmed = confirmStatus === 'confirmed'

      return (
        <div className="requirement-phase-toolbar">
          {!isConfirmed && !isAbandoned && (
            <>
              <button
                type="button"
                className="action-btn"
                onClick={() => setConfirmConfirmOpen(true)}
                disabled={busy || confirming || !isPhaseActive}
              >
                {confirming ? t('ui.requirements.confirm.processing') : t('ui.requirements.confirm.confirmAction')}
              </button>
              <button
                type="button"
                className="action-btn"
                onClick={() => setConfirmAbandonOpen(true)}
                disabled={busy || abandoning || !isPhaseActive}
              >
                {abandoning ? t('ui.requirements.confirm.processing') : t('ui.requirements.confirm.abandonAction')}
              </button>
            </>
          )}
          {isAbandoned && (
            <button
              type="button"
              className="action-btn"
              onClick={() => setConfirmReconfirmOpen(true)}
              disabled={busy || reconfirming || !isPhaseActive}
            >
              {reconfirming ? t('ui.requirements.confirm.processing') : t('ui.requirements.confirm.reconfirmAction')}
            </button>
          )}
          {isConfirmed && (
            <button type="button" className="action-btn" disabled>
              {t('ui.requirements.confirm.statusConfirmed')}
            </button>
          )}
          {confirmConfirmOpen ? (
            <div className="requirement-review-confirm" role="dialog" aria-modal="true">
              <p className="requirement-review-confirm__text">{t('ui.requirements.confirm.confirmDialogText')}</p>
              <div className="requirement-review-confirm__actions">
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    setConfirmConfirmOpen(false)
                    setConfirmActionError('')
                  }}
                >
                  {t('ui.requirements.review.cancel')}
                </button>
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    if (!detail) return
                    setConfirmActionError('')
                    void confirmRequirement(detail.id)
                      .then(() => {
                        setConfirmConfirmOpen(false)
                        setViewPhase(detail.phase === 'confirm' ? 'development' : detail.phase)
                      })
                      .catch((e) => setConfirmActionError(e instanceof Error ? e.message : String(e)))
                  }}
                  disabled={confirming}
                >
                  {t('ui.requirements.confirm.confirmDialogTitle')}
                </button>
              </div>
              {confirmActionError ? <p className="workspace-setup__error">{confirmActionError}</p> : null}
            </div>
          ) : null}
          {confirmAbandonOpen ? (
            <div className="requirement-review-confirm" role="dialog" aria-modal="true">
              <p className="requirement-review-confirm__text">{t('ui.requirements.confirm.abandonDialogText')}</p>
              <div className="requirement-review-confirm__actions">
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    setConfirmAbandonOpen(false)
                    setAbandonActionError('')
                  }}
                >
                  {t('ui.requirements.review.cancel')}
                </button>
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    if (!detail) return
                    setAbandonActionError('')
                    void abandonRequirement(detail.id)
                      .then(() => setConfirmAbandonOpen(false))
                      .catch((e) => setAbandonActionError(e instanceof Error ? e.message : String(e)))
                  }}
                  disabled={abandoning}
                >
                  {t('ui.requirements.confirm.abandonDialogTitle')}
                </button>
              </div>
              {abandonActionError ? <p className="workspace-setup__error">{abandonActionError}</p> : null}
            </div>
          ) : null}
          {confirmReconfirmOpen ? (
            <div className="requirement-review-confirm" role="dialog" aria-modal="true">
              <p className="requirement-review-confirm__text">{t('ui.requirements.confirm.reconfirmDialogText')}</p>
              <div className="requirement-review-confirm__actions">
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    setConfirmReconfirmOpen(false)
                    setReconfirmActionError('')
                  }}
                >
                  {t('ui.requirements.review.cancel')}
                </button>
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    if (!detail) return
                    setReconfirmActionError('')
                    void reconfirmRequirement(detail.id)
                      .then(() => {
                        setConfirmReconfirmOpen(false)
                      })
                      .catch((e) => setReconfirmActionError(e instanceof Error ? e.message : String(e)))
                  }}
                  disabled={reconfirming}
                >
                  {t('ui.requirements.confirm.reconfirmDialogTitle')}
                </button>
              </div>
              {reconfirmActionError ? <p className="workspace-setup__error">{reconfirmActionError}</p> : null}
            </div>
          ) : null}
        </div>
      )
    }

    // development phase: start development, create task
    if (phase === 'development') {
      const dev = requirements.development
      const featureBranchCreated = dev?.feature_branch_created

      return (
        <div className="requirement-phase-toolbar">
          {!featureBranchCreated && (
            <button
              type="button"
              className="action-btn"
              onClick={() => void requirements.startDevelopmentAction(detail!.id)}
              disabled={busy || requirements.devStarting}
            >
              {requirements.devStarting ? t('ui.requirements.development.processing') : t('ui.requirements.development.start')}
            </button>
          )}
          {featureBranchCreated && (
            <button
              type="button"
              className="action-btn"
              onClick={() => {
                setCreateTaskOpen(true)
                setCreateError('')
              }}
            >
              {t('ui.requirements.development.createTask')}
            </button>
          )}
          {createTaskOpen ? (
            <div className="requirement-review-confirm" role="dialog" aria-modal="true">
              <p className="requirement-review-confirm__text">{t('ui.requirements.development.createTaskText')}</p>
              <input
                type="text"
                className="workspace-setup__input"
                placeholder={t('ui.requirements.development.taskTitle')}
                value={createTaskTitle}
                onChange={(e) => setCreateTaskTitle(e.target.value)}
                autoFocus
              />
              <input
                type="text"
                className="workspace-setup__input"
                placeholder={t('ui.requirements.development.taskAssignee')}
                value={createTaskAssignee}
                onChange={(e) => setCreateTaskAssignee(e.target.value)}
              />
              <div className="requirement-review-confirm__actions">
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    setCreateTaskOpen(false)
                    setCreateError('')
                  }}
                >
                  {t('ui.requirements.development.cancel')}
                </button>
                <button
                  type="button"
                  className="action-btn"
                  onClick={() => {
                    if (!createTaskTitle.trim()) return
                    setCreateError('')
                    void requirements.createDevTaskAction(detail!.id, {
                      title: createTaskTitle.trim(),
                      assignee: createTaskAssignee.trim() || undefined,
                    })
                      .then(() => setCreateTaskOpen(false))
                      .catch((e) => setCreateError(e instanceof Error ? e.message : String(e)))
                  }}
                >
                  {t('ui.requirements.development.confirm')}
                </button>
              </div>
              {createError ? <p className="workspace-setup__error">{createError}</p> : null}
            </div>
          ) : null}
        </div>
      )
    }

    // testing, release: placeholder toolbar (save button)
    return (
      <div className="requirement-phase-toolbar">
        <button
          type="button"
          className="action-btn"
          onClick={() => void onSave()}
          disabled={busy || !dirty || !isPhaseActive}
        >
          {busy ? t('ui.requirements.saving') : t('ui.requirements.save')}
        </button>
      </div>
    )
  }

  if (loading && !detail) {
    return <div className="requirement-detail requirement-detail--empty">{t('ui.requirements.loading')}</div>
  }

  if (!detail) {
    return <div className="requirement-detail requirement-detail--empty">{t('ui.requirements.selectHint')}</div>
  }

  return (
    <div className="requirement-detail">
      <header className="requirement-detail__header">
        <input
          className="requirement-detail__title"
          value={titleDraft}
          onChange={(e) => {
            setTitleDraft(e.target.value)
            markDirty()
          }}
          aria-label={t('ui.requirements.titleLabel')}
        />
        <section className="requirement-detail__timeline-wrap">
          <RequirementPhaseTimeline
            phase={detail.phase}
            viewPhase={viewPhase}
            phaseLabel={phaseLabel}
            disabled={busy}
            onViewPhaseChange={(next) => {
              setViewPhase(next)
            }}
          />
        </section>
        {renderPhaseToolbar()}
        {error || saveError ? (
          <p className="workspace-setup__error">{saveError || error}</p>
        ) : null}
      </header>
      <RequirementPhaseContent
        viewPhase={viewPhase}
        phaseLabel={phaseLabel}
        contentDraft={contentDraft}
        onContentChange={(value) => {
          setContentDraft(value)
          markDirty()
        }}
        requirements={requirements}
        t={t}
      />
    </div>
  )
}
