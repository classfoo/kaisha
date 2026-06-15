import * as React from 'react'
import {
  buildOffsets,
  computeRange,
  findItemAtOffset,
  isPinnedToBottom,
  type VirtualItem,
} from './virtualRange'

export type ScrollAlign = 'start' | 'center' | 'end' | 'auto'
export type ScrollBehaviorOpt = 'auto' | 'smooth'

export type UseDynamicVirtualListOptions<T> = {
  items: T[]
  /** Stable identity per item. Measurements are cached by this key, so it must
   *  survive re-renders, insertions and streaming updates. */
  getKey: (item: T, index: number) => string
  /** Best guess for an unmeasured row (px). Closer guesses reduce scrollbar
   *  drift before real measurements land. */
  estimateHeight?: number
  /** Extra rows rendered above/below the viewport for smoother scrolling. */
  overscan?: number
  /** Vertical gap between rows (px). Folded into offsets so positioning and
   *  total height stay consistent. */
  gap?: number
  /** Distance from the bottom (px) under which the list is "pinned" and will
   *  auto-follow new content. */
  pinThreshold?: number
}

export type UseDynamicVirtualListResult = {
  scrollRef: React.RefObject<HTMLDivElement>
  viewportRef: React.RefObject<HTMLDivElement>
  /** Total scrollable content height (px). */
  totalSize: number
  virtualItems: VirtualItem[]
  /** Ref callback factory used to measure each rendered row. */
  measureElement: (key: string) => (el: HTMLElement | null) => void
  /** True while the view is following the bottom of the list. */
  pinnedToBottom: boolean
  /** True when there is hidden content below the viewport. */
  hasContentBelow: boolean
  scrollToBottom: (behavior?: ScrollBehaviorOpt) => void
  scrollToIndex: (index: number, opts?: { align?: ScrollAlign; behavior?: ScrollBehaviorOpt }) => void
  /** Attach to the scroll container's `onScroll`. */
  onScroll: () => void
}

const DEFAULT_ESTIMATE = 96
const DEFAULT_OVERSCAN = 6
const DEFAULT_GAP = 8
const DEFAULT_PIN_THRESHOLD = 48

/**
 * Dynamic-height windowing for very long lists.
 *
 * Unlike the previous fixed-row approach, every rendered row is measured with a
 * shared `ResizeObserver`; the measured heights drive an offset table that keeps
 * the scrollbar accurate and the visible window correct for arbitrary content.
 *
 * Two behaviors make scrolling feel smooth:
 *  - **Scroll anchoring**: when off-screen/overscan rows measure for the first
 *    time (or change height while streaming), the topmost visible row is held in
 *    place by compensating `scrollTop`, eliminating the classic "content jumps
 *    while I scroll" problem.
 *  - **Sticky bottom**: while pinned near the bottom, new/growing content keeps
 *    the view glued to the latest message; scrolling up releases the pin.
 */
export function useDynamicVirtualList<T>(
  options: UseDynamicVirtualListOptions<T>,
): UseDynamicVirtualListResult {
  const {
    items,
    getKey,
    estimateHeight = DEFAULT_ESTIMATE,
    overscan = DEFAULT_OVERSCAN,
    gap = DEFAULT_GAP,
    pinThreshold = DEFAULT_PIN_THRESHOLD,
  } = options

  const scrollRef = React.useRef<HTMLDivElement>(null)
  const viewportRef = React.useRef<HTMLDivElement>(null)

  const keys = React.useMemo(() => items.map((item, i) => getKey(item, i)), [items, getKey])

  // Measured heights survive across renders, keyed by stable item key.
  const heightsRef = React.useRef<Map<string, number>>(new Map())
  // Bumped whenever a measurement changes, to recompute offsets/range.
  const [measureVersion, setMeasureVersion] = React.useState(0)

  const [scrollTop, setScrollTop] = React.useState(0)
  const [viewportHeight, setViewportHeight] = React.useState(0)
  // Offset of the virtualized content within the scroll container (header, padding).
  const listStartRef = React.useRef(0)

  const effectiveHeight = React.useCallback(
    (key: string): number => (heightsRef.current.get(key) ?? estimateHeight) + gap,
    [estimateHeight, gap],
  )

  // Offset table recomputed when keys or measurements change.
  const offsets = React.useMemo(
    () => buildOffsets(keys.length, (i) => effectiveHeight(keys[i])),
    // measureVersion is an intentional recompute trigger.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [keys, effectiveHeight, measureVersion],
  )
  const offsetsRef = React.useRef(offsets)
  offsetsRef.current = offsets

  const totalSize = offsets[offsets.length - 1] ?? 0

  const indexByKey = React.useMemo(() => {
    const map = new Map<string, number>()
    for (let i = 0; i < keys.length; i++) map.set(keys[i], i)
    return map
  }, [keys])

  const relativeScrollTop = Math.max(0, scrollTop - listStartRef.current)
  const range = computeRange({ offsets, scrollTop: relativeScrollTop, viewportHeight, overscan })

  const virtualItems = React.useMemo<VirtualItem[]>(() => {
    const out: VirtualItem[] = []
    for (let i = range.startIndex; i < range.endIndex; i++) {
      out.push({
        index: i,
        key: keys[i],
        start: offsets[i],
        size: effectiveHeight(keys[i]) - gap,
      })
    }
    return out
  }, [range.startIndex, range.endIndex, keys, offsets, effectiveHeight, gap])

  // ---- Pin-to-bottom tracking ---------------------------------------------
  const pinnedRef = React.useRef(true)
  const [pinnedToBottom, setPinnedToBottom] = React.useState(true)
  const [hasContentBelow, setHasContentBelow] = React.useState(false)

  const readScrollState = React.useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    const pinned = isPinnedToBottom(
      { scrollTop: el.scrollTop, scrollHeight: el.scrollHeight, clientHeight: el.clientHeight },
      pinThreshold,
    )
    pinnedRef.current = pinned
    setPinnedToBottom(pinned)
    setHasContentBelow(el.scrollHeight - el.scrollTop - el.clientHeight > 1)
  }, [pinThreshold])

  // ---- Scroll anchor (held while measurements shift offsets) --------------
  const anchorRef = React.useRef<{ key: string; top: number } | null>(null)

  const rafRef = React.useRef<number | null>(null)
  const handleScroll = React.useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    if (rafRef.current != null) return
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = null
      setScrollTop(el.scrollTop)
      readScrollState()
    })
  }, [readScrollState])

  const scrollToBottom = React.useCallback((behavior: ScrollBehaviorOpt = 'auto') => {
    const el = scrollRef.current
    if (!el) return
    el.scrollTo({ top: el.scrollHeight, behavior })
    pinnedRef.current = true
    setPinnedToBottom(true)
  }, [])

  const scrollToIndex = React.useCallback(
    (index: number, opts?: { align?: ScrollAlign; behavior?: ScrollBehaviorOpt }) => {
      const el = scrollRef.current
      if (!el) return
      const offs = offsetsRef.current
      const count = offs.length - 1
      if (count <= 0) return
      const clamped = Math.max(0, Math.min(count - 1, index))
      const align = opts?.align ?? 'auto'
      const itemTop = listStartRef.current + offs[clamped]
      const itemBottom = listStartRef.current + offs[clamped + 1]
      const viewTop = el.scrollTop
      const viewBottom = viewTop + el.clientHeight

      let target = viewTop
      if (align === 'start') target = itemTop
      else if (align === 'end') target = itemBottom - el.clientHeight
      else if (align === 'center') target = itemTop - (el.clientHeight - (itemBottom - itemTop)) / 2
      else {
        // auto: only scroll if the item is outside the viewport
        if (itemTop < viewTop) target = itemTop
        else if (itemBottom > viewBottom) target = itemBottom - el.clientHeight
        else return
      }
      el.scrollTo({ top: Math.max(0, target), behavior: opts?.behavior ?? 'auto' })
    },
    [],
  )

  // ---- Measurement via a shared ResizeObserver ----------------------------
  const elToKeyRef = React.useRef<WeakMap<Element, string>>(new WeakMap())
  const observerRef = React.useRef<ResizeObserver | null>(null)
  const flushScheduledRef = React.useRef(false)

  const commitHeight = React.useCallback((key: string, height: number) => {
    const prev = heightsRef.current.get(key)
    if (prev !== undefined && Math.abs(prev - height) < 0.5) return
    heightsRef.current.set(key, height)
    if (!flushScheduledRef.current) {
      flushScheduledRef.current = true
      requestAnimationFrame(() => {
        flushScheduledRef.current = false
        setMeasureVersion((v) => v + 1)
      })
    }
  }, [])

  const getObserver = React.useCallback((): ResizeObserver | null => {
    if (typeof ResizeObserver === 'undefined') return null
    if (!observerRef.current) {
      observerRef.current = new ResizeObserver((entries) => {
        for (const entry of entries) {
          const key = elToKeyRef.current.get(entry.target)
          if (!key) continue
          commitHeight(key, (entry.target as HTMLElement).offsetHeight)
        }
      })
    }
    return observerRef.current
  }, [commitHeight])

  const keyToElRef = React.useRef<Map<string, HTMLElement>>(new Map())
  const measureCallbacks = React.useRef<Map<string, (el: HTMLElement | null) => void>>(new Map())
  const measureElement = React.useCallback(
    (key: string) => {
      let cb = measureCallbacks.current.get(key)
      if (!cb) {
        cb = (el: HTMLElement | null) => {
          const observer = getObserver()
          const prevEl = keyToElRef.current.get(key)
          if (prevEl && prevEl !== el) {
            observer?.unobserve(prevEl)
            keyToElRef.current.delete(key)
          }
          if (el) {
            keyToElRef.current.set(key, el)
            elToKeyRef.current.set(el, key)
            observer?.observe(el)
            commitHeight(key, el.offsetHeight)
          }
        }
        measureCallbacks.current.set(key, cb)
      }
      return cb
    },
    [getObserver, commitHeight],
  )

  React.useEffect(() => {
    return () => {
      observerRef.current?.disconnect()
      observerRef.current = null
      if (rafRef.current != null) cancelAnimationFrame(rafRef.current)
    }
  }, [])

  // ---- Track viewport size ------------------------------------------------
  React.useEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const update = () => {
      setViewportHeight(el.clientHeight)
      const vp = viewportRef.current
      listStartRef.current = vp ? vp.offsetTop : 0
      readScrollState()
    }
    update()
    if (typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(update)
    ro.observe(el)
    return () => ro.disconnect()
  }, [readScrollState])

  // ---- Layout pass: keep the view stable across offset changes ------------
  React.useLayoutEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const vp = viewportRef.current
    if (vp) listStartRef.current = vp.offsetTop
    const offs = offsetsRef.current
    const listStart = listStartRef.current

    if (pinnedRef.current) {
      // Sticky bottom: follow new/growing content instantly.
      el.scrollTop = el.scrollHeight
    } else {
      const anchor = anchorRef.current
      if (anchor) {
        const idx = indexByKey.get(anchor.key)
        if (idx !== undefined) {
          const newTop = listStart + offs[idx]
          const delta = newTop - anchor.top
          if (Math.abs(delta) > 0.5) {
            el.scrollTop += delta
          }
        }
      }
    }

    // Recompute the anchor from the (possibly adjusted) scroll position.
    const rel = Math.max(0, el.scrollTop - listStart)
    const anchorIndex = findItemAtOffset(offs, rel)
    const anchorKey = keys[anchorIndex]
    if (anchorKey !== undefined) {
      anchorRef.current = { key: anchorKey, top: listStart + offs[anchorIndex] }
    }

    if (Math.abs(el.scrollTop - scrollTop) > 0.5) setScrollTop(el.scrollTop)
    readScrollState()
    // Re-run whenever offsets (measureVersion) or the item set change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [measureVersion, totalSize, keys])

  return {
    scrollRef,
    viewportRef,
    totalSize,
    virtualItems,
    measureElement,
    pinnedToBottom,
    hasContentBelow,
    scrollToBottom,
    scrollToIndex,
    onScroll: handleScroll,
  }
}
