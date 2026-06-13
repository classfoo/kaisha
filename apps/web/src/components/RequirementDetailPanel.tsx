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
  const { detail, loading, busy, error, saveRequirement } = requirements
  const [titleDraft, setTitleDraft] = React.useState('')
  const [phaseDraft, setPhaseDraft] = React.useState<RequirementPhase>('collection')
  const [viewPhase, setViewPhase] = React.useState<RequirementPhase>('collection')
  const [contentDraft, setContentDraft] = React.useState('')
  const [dirty, setDirty] = React.useState(false)
  const [saveError, setSaveError] = React.useState('')
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

    // collection phase: save button
    if (phase === 'collection') {
      return (
        <div className="requirement-phase-toolbar">
          <button
            type="button"
            className="action-btn"
            onClick={() => void onSave()}
            disabled={busy || !dirty}
          >
            {busy ? t('ui.requirements.saving') : t('ui.requirements.save')}
          </button>
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
          disabled={busy || !dirty}
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
              setPhaseDraft(next)
              markDirty()
            }}
          />
        </section>
        {dirty ? (
          <div className="requirement-phase-toolbar requirement-phase-toolbar--save">
            <button
              type="button"
              className="action-btn"
              onClick={() => void onSave()}
              disabled={busy}
            >
              {busy ? t('ui.requirements.saving') : t('ui.requirements.save')}
            </button>
          </div>
        ) : null}
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
