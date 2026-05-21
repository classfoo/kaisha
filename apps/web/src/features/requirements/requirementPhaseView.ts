import type { RequirementPhase } from './requirementsApi'

export type RequirementPhaseViewKind = 'collection' | 'review' | 'confirm' | 'placeholder'

export function phaseViewKind(phase: RequirementPhase): RequirementPhaseViewKind {
  if (phase === 'collection') return 'collection'
  if (phase === 'review') return 'review'
  if (phase === 'confirm') return 'confirm'
  return 'placeholder'
}
