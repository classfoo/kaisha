import type { RequirementPhase } from './requirementsApi'

export type RequirementPhaseViewKind =
  | 'collection'
  | 'development'
  | 'testing'
  | 'release'
  | 'placeholder'

export function phaseViewKind(phase: RequirementPhase): RequirementPhaseViewKind {
  if (phase === 'collection') return 'collection'
  if (phase === 'development') return 'development'
  if (phase === 'testing') return 'testing'
  if (phase === 'release') return 'release'
  return 'placeholder'
}
