import type { RequirementPhase } from './requirementsApi'

export type RequirementPhaseViewKind = 'collection' | 'development' | 'placeholder'

export function phaseViewKind(phase: RequirementPhase): RequirementPhaseViewKind {
  if (phase === 'collection') return 'collection'
  if (phase === 'development') return 'development'
  return 'placeholder'
}
