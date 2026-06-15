import * as React from 'react'
import { useDynamicVirtualList } from '../features/virtual-list/useDynamicVirtualList'

export type ChatHistoryListHandle = {
  scrollToBottom: (behavior?: 'auto' | 'smooth') => void
  scrollToIndex: (index: number, opts?: { align?: 'start' | 'center' | 'end' | 'auto'; behavior?: 'auto' | 'smooth' }) => void
}

type ChatHistoryListProps<T> = {
  items: T[]
  getKey: (item: T, index: number) => string
  renderItem: (item: T, index: number) => React.ReactNode
  /** Optional content rendered above the list (e.g. loading / result banners). */
  header?: React.ReactNode
  /** Label for the floating "jump to latest" affordance. */
  scrollDownLabel: string
  estimateHeight?: number
  className?: string
  /** Whether there are more messages available to load above. */
  hasMoreAbove?: boolean
  /** Whether older messages are currently being loaded. */
  loadingMore?: boolean
  /** Callback to load more messages. */
  onLoadMore?: () => void
  /** Label for the load more button. */
  loadMoreLabel?: string
}

/**
 * Scroll container for a (potentially very large) chat history.
 *
 * Rendering, measuring, scroll anchoring and sticky-bottom behavior are all
 * delegated to `useDynamicVirtualList`. Rows are absolutely positioned inside a
 * single sized "viewport" element so the scrollbar reflects the true content
 * height while only a small window of rows is mounted at any time.
 */
function ChatHistoryListInner<T>(
  { items, getKey, renderItem, header, scrollDownLabel, estimateHeight, className, hasMoreAbove, loadingMore, onLoadMore, loadMoreLabel }: ChatHistoryListProps<T>,
  ref: React.Ref<ChatHistoryListHandle>,
) {
  const {
    scrollRef,
    viewportRef,
    totalSize,
    virtualItems,
    measureElement,
    pinnedToBottom,
    hasContentBelow,
    scrollToBottom,
    scrollToIndex,
    onScroll,
  } = useDynamicVirtualList<T>({ items, getKey, estimateHeight })

  React.useImperativeHandle(ref, () => ({ scrollToBottom, scrollToIndex }), [scrollToBottom, scrollToIndex])

  const showJumpButton = hasContentBelow && !pinnedToBottom

  return (
    <div className="chat-history-wrap">
      <div className={className ?? 'chat-history'} ref={scrollRef} onScroll={onScroll}>
        {header}
        {hasMoreAbove && onLoadMore ? (
          <div className="chat-history__load-more">
            <button
              type="button"
              className="chat-history__load-more-btn"
              onClick={onLoadMore}
              disabled={loadingMore}
              aria-label={loadMoreLabel}
            >
              {loadingMore ? '...' : (loadMoreLabel || 'Load more')}
            </button>
          </div>
        ) : null}
        <div className="chat-history__viewport" ref={viewportRef} style={{ height: totalSize, position: 'relative' }}>
          {virtualItems.map((vi) => (
            <div
              key={vi.key}
              ref={measureElement(vi.key)}
              className="chat-history__row"
              style={{ position: 'absolute', top: vi.start, left: 0, right: 0 }}
            >
              {renderItem(items[vi.index], vi.index)}
            </div>
          ))}
        </div>
      </div>
      {showJumpButton ? (
        <button
          type="button"
          className="chat-scroll-down"
          onClick={() => scrollToBottom('smooth')}
          title={scrollDownLabel}
          aria-label={scrollDownLabel}
        >
          <span aria-hidden="true">↓</span>
        </button>
      ) : null}
    </div>
  )
}

/** Generic-friendly forwardRef wrapper. */
export const ChatHistoryList = React.forwardRef(ChatHistoryListInner) as <T>(
  props: ChatHistoryListProps<T> & { ref?: React.Ref<ChatHistoryListHandle> },
) => React.ReactElement
