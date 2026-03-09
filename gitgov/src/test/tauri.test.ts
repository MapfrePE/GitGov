import { describe, it, expect } from 'vitest'
import { parseCommandError } from '@/lib/tauri'

describe('parseCommandError', () => {
  it('parses a valid JSON error with code and message', () => {
    const input = JSON.stringify({ code: 'BLOCKED', message: 'Branch protegida' })
    const result = parseCommandError(input)
    expect(result.code).toBe('BLOCKED')
    expect(result.message).toBe('Branch protegida')
  })

  it('parses JSON missing code field → defaults to UNKNOWN', () => {
    const input = JSON.stringify({ message: 'algo falló' })
    const result = parseCommandError(input)
    expect(result.code).toBe('UNKNOWN')
    expect(result.message).toBe('algo falló')
  })

  it('parses JSON missing message field → uses raw string', () => {
    const input = JSON.stringify({ code: 'ERR' })
    const result = parseCommandError(input)
    expect(result.code).toBe('ERR')
    expect(result.message).toBe(input)
  })

  it('handles plain string (non-JSON) → UNKNOWN code + raw message', () => {
    const result = parseCommandError('something went wrong')
    expect(result.code).toBe('UNKNOWN')
    expect(result.message).toBe('something went wrong')
  })

  it('handles empty string → UNKNOWN code + empty message', () => {
    const result = parseCommandError('')
    expect(result.code).toBe('UNKNOWN')
    expect(result.message).toBe('')
  })

  it('handles GOVERNANCE_BLOCKED code', () => {
    const input = JSON.stringify({ code: 'GOVERNANCE_BLOCKED', message: 'Requires PR' })
    const result = parseCommandError(input)
    expect(result.code).toBe('GOVERNANCE_BLOCKED')
    expect(result.message).toBe('Requires PR')
  })
})
