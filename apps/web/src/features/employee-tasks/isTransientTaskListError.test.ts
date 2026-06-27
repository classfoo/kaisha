import { describe, expect, it } from 'vitest'
import { isTransientTaskListError } from './isTransientTaskListError'

describe('isTransientTaskListError', () => {
  it('treats WebKit "Load failed" as transient', () => {
    expect(isTransientTaskListError(new TypeError('Load failed'))).toBe(true)
  })

  it('treats Chromium fetch failures as transient', () => {
    expect(isTransientTaskListError(new TypeError('Failed to fetch'))).toBe(true)
  })

  it('does not treat validation errors as transient', () => {
    expect(isTransientTaskListError(new Error('employee_not_found'))).toBe(false)
  })
})
