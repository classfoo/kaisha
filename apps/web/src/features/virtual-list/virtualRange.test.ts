import { describe, expect, it } from 'vitest'
import {
  buildOffsets,
  computeRange,
  distanceFromBottom,
  findItemAtOffset,
  isPinnedToBottom,
} from './virtualRange'

describe('buildOffsets', () => {
  it('returns [0] for an empty list', () => {
    expect(buildOffsets(0, () => 100)).toEqual([0])
  })

  it('accumulates variable heights into a prefix-sum table', () => {
    const heights = [50, 120, 30, 200]
    const offsets = buildOffsets(heights.length, (i) => heights[i])
    expect(offsets).toEqual([0, 50, 170, 200, 400])
    // total content height is the last entry
    expect(offsets[offsets.length - 1]).toBe(400)
  })

  it('treats negative / non-finite heights as zero', () => {
    const offsets = buildOffsets(3, (i) => [100, -20, Number.NaN][i])
    expect(offsets).toEqual([0, 100, 100, 100])
  })
})

describe('findItemAtOffset', () => {
  const offsets = buildOffsets(4, (i) => [50, 120, 30, 200][i]) // [0,50,170,200,400]

  it('finds the item whose box contains the position', () => {
    expect(findItemAtOffset(offsets, 0)).toBe(0)
    expect(findItemAtOffset(offsets, 49)).toBe(0)
    expect(findItemAtOffset(offsets, 50)).toBe(1) // boundary belongs to next item
    expect(findItemAtOffset(offsets, 169)).toBe(1)
    expect(findItemAtOffset(offsets, 170)).toBe(2)
    expect(findItemAtOffset(offsets, 199)).toBe(2)
    expect(findItemAtOffset(offsets, 200)).toBe(3)
  })

  it('clamps out-of-range positions to valid indices', () => {
    expect(findItemAtOffset(offsets, -100)).toBe(0)
    expect(findItemAtOffset(offsets, 99999)).toBe(3)
  })

  it('handles an empty offsets table', () => {
    expect(findItemAtOffset([0], 10)).toBe(0)
  })
})

describe('computeRange', () => {
  // 1000 items of 100px each => content height 100_000
  const offsets = buildOffsets(1000, () => 100)

  it('renders only the visible window plus overscan', () => {
    const range = computeRange({ offsets, scrollTop: 10_000, viewportHeight: 500, overscan: 5 })
    // visible items: 100..105 (6 row boundaries across 500px). With overscan 5 on each side.
    expect(range.startIndex).toBe(95)
    expect(range.endIndex).toBe(111)
    // window stays tiny regardless of list size -> O(1) DOM nodes
    expect(range.endIndex - range.startIndex).toBeLessThan(20)
  })

  it('clamps the window at the top of the list', () => {
    const range = computeRange({ offsets, scrollTop: 0, viewportHeight: 500, overscan: 5 })
    expect(range.startIndex).toBe(0)
    expect(range.endIndex).toBe(11)
  })

  it('clamps the window at the bottom of the list', () => {
    const range = computeRange({ offsets, scrollTop: 99_500, viewportHeight: 500, overscan: 5 })
    expect(range.endIndex).toBe(1000)
    expect(range.startIndex).toBeGreaterThan(980)
  })

  it('returns an empty range for an empty list', () => {
    expect(computeRange({ offsets: [0], scrollTop: 0, viewportHeight: 500, overscan: 5 })).toEqual({
      startIndex: 0,
      endIndex: 0,
    })
  })

  it('works with variable heights', () => {
    // item 0 is huge, the rest are short
    const variable = buildOffsets(5, (i) => (i === 0 ? 1000 : 50))
    // [0, 1000, 1050, 1100, 1150, 1200]
    const range = computeRange({ offsets: variable, scrollTop: 1040, viewportHeight: 100, overscan: 0 })
    expect(range.startIndex).toBe(1)
    expect(range.endIndex).toBe(4)
  })
})

describe('distanceFromBottom / isPinnedToBottom', () => {
  it('measures the gap to the bottom of the scroll content', () => {
    expect(distanceFromBottom({ scrollTop: 800, scrollHeight: 1000, clientHeight: 200 })).toBe(0)
    expect(distanceFromBottom({ scrollTop: 700, scrollHeight: 1000, clientHeight: 200 })).toBe(100)
  })

  it('never returns a negative distance (overscroll/bounce)', () => {
    expect(distanceFromBottom({ scrollTop: 900, scrollHeight: 1000, clientHeight: 200 })).toBe(0)
  })

  it('considers the view pinned within the threshold', () => {
    expect(isPinnedToBottom({ scrollTop: 760, scrollHeight: 1000, clientHeight: 200 }, 50)).toBe(true)
    expect(isPinnedToBottom({ scrollTop: 700, scrollHeight: 1000, clientHeight: 200 }, 50)).toBe(false)
  })
})
