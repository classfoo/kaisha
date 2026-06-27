/** Determines if a task-list fetch error is transient (worth retrying silently). */
export function isTransientTaskListError(error: Error): boolean {
  const msg = error.message.toLowerCase()
  return (
    msg.includes('fetch') ||
    msg.includes('network') ||
    msg.includes('load failed') ||
    msg.includes('failed to fetch') ||
    msg.includes('networkerror') ||
    msg.includes('internal server error') ||
    msg.includes('500') ||
    msg.includes('502') ||
    msg.includes('503') ||
    msg.includes('504') ||
    msg.includes('task_load_failed') ||
    msg.includes('skipping')
  )
}
