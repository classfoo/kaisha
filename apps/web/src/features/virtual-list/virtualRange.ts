/**
 * Pure, framework-agnostic math for a dynamic (variable-height) virtualized
 * list. Keeping this logic free of React/DOM makes the tricky parts (offset
 * accounting, visible-range search, scroll anchoring) unit-testable in
 * isolation, which is exactly where the previous fixed-height implementation
 * silently went wrong.
 */

export type VirtualRange = {
  /** First index to render (inclusive). */
  startIndex: number
  /** Last index to render (exclusive). */
  endIndex: number
}

export type VirtualItem = {
  index: number
  key: string
  /** Top offset of the item within the virtualized content, in px. */
  start: number
  /** Measured (or estimated) height of the item, in px. */
  size: number
}

/**
 * Build the cumulative-offset table for `count` items.
 *
 * The returned array has length `count + 1`: `offsets[i]` is the top edge of
 * item `i`, and `offsets[count]` is the total content height. Each item's
 * effective height is `max(0, getHeight(i))` and already includes any inter-item
 * gap the caller folds into `getHeight`.
 */
export function buildOffsets(count: number, getHeight: (index: number) => number): number[] {
  const safeCount = Math.max(0, count)
  const offsets = new Array<number>(safeCount + 1)
  offsets[0] = 0
  for (let i = 0; i < safeCount; i++) {
    const h = getHeight(i)
    offsets[i + 1] = offsets[i] + (Number.isFinite(h) && h > 0 ? h : 0)
  }
  return offsets
}

/**
 * Find the index of the item that contains the vertical `position` (i.e. the
 * first item whose bottom edge is strictly greater than `position`). Runs a
 * binary search over the monotonically increasing offset table, so it stays
 * O(log n) even for very large histories. The result is clamped to a valid
 * item index.
 */
export function findItemAtOffset(offsets: number[], position: number): number {
  const count = offsets.length - 1
  if (count <= 0) return 0
  const target = position < 0 ? 0 : position
  let lo = 0
  let hi = count - 1
  let ans = count - 1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (offsets[mid + 1] > target) {
      ans = mid
      hi = mid - 1
    } else {
      lo = mid + 1
    }
  }
  return ans
}

/**
 * Compute the [startIndex, endIndex) window of items that should be rendered for
 * a given scroll position and viewport height, padded by `overscan` items on
 * each side to keep scrolling smooth.
 */
export function computeRange(params: {
  offsets: number[]
  scrollTop: number
  viewportHeight: number
  overscan: number
}): VirtualRange {
  const { offsets, scrollTop, viewportHeight, overscan } = params
  const count = offsets.length - 1
  if (count <= 0) return { startIndex: 0, endIndex: 0 }

  const overscanSafe = Math.max(0, Math.floor(overscan))
  const top = scrollTop < 0 ? 0 : scrollTop
  const bottom = top + Math.max(0, viewportHeight)

  const firstVisible = findItemAtOffset(offsets, top)
  const lastVisible = findItemAtOffset(offsets, bottom)

  const startIndex = Math.max(0, firstVisible - overscanSafe)
  const endIndex = Math.min(count, lastVisible + 1 + overscanSafe)
  return { startIndex, endIndex }
}

/**
 * Distance (in px) from the bottom of the scrollable content. Used to decide
 * whether the view is "pinned" to the bottom (chat-style sticky behavior).
 */
export function distanceFromBottom(params: {
  scrollTop: number
  scrollHeight: number
  clientHeight: number
}): number {
  const { scrollTop, scrollHeight, clientHeight } = params
  return Math.max(0, scrollHeight - scrollTop - clientHeight)
}

/**
 * Whether the view should be considered pinned to the bottom given a threshold.
 */
export function isPinnedToBottom(
  params: { scrollTop: number; scrollHeight: number; clientHeight: number },
  threshold: number,
): boolean {
  return distanceFromBottom(params) <= Math.max(0, threshold)
}
