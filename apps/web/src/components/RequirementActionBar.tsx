import React from 'react'
import type { useRequirementsWorkspace } from '../features/requirements/useRequirementsWorkspace'
import type { RequirementPhase } from '../features/requirements/requirementsApi'

type RequirementActionBarProps = {
  requirements: ReturnType<typeof useRequirementsWorkspace>
  viewPhase: RequirementPhase
  t: (key: string) => string
}

/**
 * Unified action bar placed below the progress timeline.
 * Only shows **global** operations for the current phase -- not per-task actions.
 * Per-task actions remain in their respective task rows.
 */
export const RequirementActionBar = React.memo(function RequirementActionBar({
  requirements,
  viewPhase,
  t,
}: RequirementActionBarProps) {
  const {
    detail,
    development,
    devStarting,
    agentActionKey,
    startDevelopmentAction,
    splitDevTasksAction,
    splitTestTasksAction,
    packageReleaseAction,
    startReleaseAction,
    optimizeAction,
    reloadTesting,
    reloadRelease,
    releaseLoading,
  } = requirements

  const isAgentBusy = (key: string) => agentActionKey === key

  // ---- Collection Phase ----
  if (viewPhase === 'collection') {
    return (
      <div className="requirement-action-bar">
        <button
          type="button"
          className="action-btn"
          onClick={() => void optimizeAction(detail!.id)}
          disabled={isAgentBusy('optimize')}
        >
          {isAgentBusy('optimize') ? t('ui.requirements.optimizing') : t('ui.requirements.optimize')}
        </button>
      </div>
    )
  }

  // ---- Development Phase ----
  if (viewPhase === 'development') {
    const featureBranchCreated = development?.feature_branch_created

    return (
      <div className="requirement-action-bar">
        {!featureBranchCreated && detail && (
          <button
            type="button"
            className="action-btn"
            onClick={() => void startDevelopmentAction(detail.id)}
            disabled={devStarting}
          >
            {devStarting ? t('ui.requirements.development.processing') : t('ui.requirements.development.start')}
          </button>
        )}
        {featureBranchCreated && (
          <button
            type="button"
            className="action-btn"
            onClick={() => void splitDevTasksAction(detail!.id)}
            disabled={isAgentBusy('splitDev')}
          >
            {isAgentBusy('splitDev') ? t('ui.requirements.development.splitting') : t('ui.requirements.development.splitTasks')}
          </button>
        )}
      </div>
    )
  }

  // ---- Testing Phase ----
  if (viewPhase === 'testing') {
    return (
      <div className="requirement-action-bar">
        <button
          type="button"
          className="action-btn"
          onClick={() => {
            if (!detail) return
            void splitTestTasksAction(detail.id)
          }}
          disabled={isAgentBusy('splitTest')}
        >
          {isAgentBusy('splitTest') ? t('ui.requirements.testing.splitting') : t('ui.requirements.testing.splitTasks')}
        </button>
        <button
          type="button"
          className="action-btn"
          onClick={() => {
            if (!detail) return
            void reloadTesting(detail.id)
          }}
        >
          {t('ui.requirements.testing.refresh')}
        </button>
      </div>
    )
  }

  // ---- Release Phase ----
  if (viewPhase === 'release') {
    return (
      <div className="requirement-action-bar">
        <button
          type="button"
          className="action-btn"
          onClick={() => void packageReleaseAction(detail!.id)}
          disabled={isAgentBusy('package')}
        >
          {isAgentBusy('package') ? t('ui.requirements.release.packaging') : t('ui.requirements.release.package')}
        </button>
        <button
          type="button"
          className="action-btn"
          onClick={() => void startReleaseAction(detail!.id)}
          disabled={isAgentBusy('start')}
        >
          {isAgentBusy('start') ? t('ui.requirements.release.starting') : t('ui.requirements.release.start')}
        </button>
        <button
          type="button"
          className="action-btn"
          onClick={() => void reloadRelease(detail!.id)}
          disabled={releaseLoading}
        >
          {releaseLoading ? t('ui.requirements.release.loading') : t('ui.requirements.release.getOutput')}
        </button>
      </div>
    )
  }

  return null
})
