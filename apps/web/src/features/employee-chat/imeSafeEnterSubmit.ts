/** Ignore Enter briefly after composition ends (IME confirm key). */
export const IME_ENTER_GUARD_MS = 80

export type ImeEnterGuardState = {
  composing: boolean
  ignoreEnterUntil: number
}

export function createImeEnterGuardState(): ImeEnterGuardState {
  return { composing: false, ignoreEnterUntil: 0 }
}

export function onCompositionStart(state: ImeEnterGuardState): void {
  state.composing = true
}

export function onCompositionEnd(state: ImeEnterGuardState, now: number = performance.now()): void {
  state.composing = false
  state.ignoreEnterUntil = now + IME_ENTER_GUARD_MS
}

/** True when Enter should submit the prompt (not IME confirm / candidate selection). */
type EnterKeyEventLike = {
  key: string
  shiftKey: boolean
  isComposing: boolean
  keyCode: number
}

export function shouldSubmitOnEnter(
  event: EnterKeyEventLike,
  state: ImeEnterGuardState,
  now: number = performance.now(),
): boolean {
  const key = event.key
  if (state.composing || event.isComposing) {
    return false
  }
  // Legacy IME / Safari: keyCode 229 or key "Process" during composition
  if (event.keyCode === 229 || key === 'Process') {
    return false
  }
  if (now < state.ignoreEnterUntil) {
    return false
  }
  if (key !== 'Enter' || event.shiftKey) {
    return false
  }
  return true
}
