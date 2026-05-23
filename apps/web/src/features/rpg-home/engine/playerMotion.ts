import { Direction, directionDelta } from './direction'

export type MotionState = {
  gridX: number
  gridY: number
  pixelX: number
  pixelY: number
  facing: Direction
  isMoving: boolean
  moveProgress: number
  fromX: number
  fromY: number
  toX: number
  toY: number
  walkFrame: number
  walkTimerMs: number
  idleFrame: number
  idleTimerMs: number
}

export const MOVE_DURATION_MS = 190
export const WALK_STEP_MS = 95
export const IDLE_CYCLE_MS = 700

function easeInOut(t: number): number {
  return t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2
}

export function createMotionState(gridX: number, gridY: number, tileSize: number): MotionState {
  return {
    gridX,
    gridY,
    pixelX: gridX * tileSize + tileSize / 2,
    pixelY: gridY * tileSize + tileSize / 2,
    facing: Direction.Down,
    isMoving: false,
    moveProgress: 0,
    fromX: gridX,
    fromY: gridY,
    toX: gridX,
    toY: gridY,
    walkFrame: 0,
    walkTimerMs: 0,
    idleFrame: 0,
    idleTimerMs: 0,
  }
}

function tryStartMove(
  state: MotionState,
  direction: Direction,
  canMoveTo: (x: number, y: number) => boolean,
): MotionState {
  const { dx, dy } = directionDelta(direction)
  const targetX = state.gridX + dx
  const targetY = state.gridY + dy
  if (!canMoveTo(targetX, targetY)) {
    return { ...state, facing: direction }
  }
  return {
    ...state,
    facing: direction,
    isMoving: true,
    moveProgress: 0,
    fromX: state.gridX,
    fromY: state.gridY,
    toX: targetX,
    toY: targetY,
    walkFrame: 0,
    walkTimerMs: 0,
  }
}

export function updatePlayerMotion(
  state: MotionState,
  inputDirection: Direction | null,
  canMoveTo: (x: number, y: number) => boolean,
  tileSize: number,
  deltaMs: number,
): MotionState {
  if (state.isMoving) {
    const next = { ...state, moveProgress: state.moveProgress + deltaMs / MOVE_DURATION_MS }
    next.walkTimerMs += deltaMs
    if (next.walkTimerMs >= WALK_STEP_MS) {
      next.walkFrame = (next.walkFrame + 1) % 4
      next.walkTimerMs = 0
    }

    const t = easeInOut(Math.min(next.moveProgress, 1))
    next.pixelX = (next.fromX + (next.toX - next.fromX) * t) * tileSize + tileSize / 2
    next.pixelY = (next.fromY + (next.toY - next.fromY) * t) * tileSize + tileSize / 2

    if (next.moveProgress < 1) {
      return next
    }

    const settled: MotionState = {
      ...next,
      gridX: next.toX,
      gridY: next.toY,
      pixelX: next.toX * tileSize + tileSize / 2,
      pixelY: next.toY * tileSize + tileSize / 2,
      isMoving: false,
      moveProgress: 0,
    }

    if (inputDirection) {
      return tryStartMove(settled, inputDirection, canMoveTo)
    }
    return settled
  }

  if (inputDirection) {
    if (inputDirection !== state.facing) {
      return tryStartMove({ ...state, facing: inputDirection }, inputDirection, canMoveTo)
    }
    return tryStartMove(state, inputDirection, canMoveTo)
  }

  const idleNext = { ...state, idleTimerMs: state.idleTimerMs + deltaMs }
  if (idleNext.idleTimerMs >= IDLE_CYCLE_MS) {
    idleNext.idleFrame = (idleNext.idleFrame + 1) % 2
    idleNext.idleTimerMs = 0
  }
  return idleNext
}
