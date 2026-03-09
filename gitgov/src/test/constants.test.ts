import { describe, it, expect } from 'vitest'
import {
  COMMIT_TYPES,
  STATUS_COLORS,
  ACTION_COLORS,
  FILE_STATUS_COLORS,
  FILE_STATUS_LABELS,
} from '@/lib/constants'

describe('constants', () => {
  describe('COMMIT_TYPES', () => {
    it('contains conventional commit types', () => {
      const values = COMMIT_TYPES.map((t) => t.value)
      expect(values).toContain('feat')
      expect(values).toContain('fix')
      expect(values).toContain('docs')
      expect(values).toContain('refactor')
      expect(values).toContain('test')
      expect(values).toContain('chore')
    })

    it('each type has value, label, and description', () => {
      for (const type of COMMIT_TYPES) {
        expect(type.value).toBeTruthy()
        expect(type.label).toBeTruthy()
        expect(type.description).toBeTruthy()
      }
    })

    it('includes hotfix type', () => {
      expect(COMMIT_TYPES.map((t) => t.value)).toContain('hotfix')
    })
  })

  describe('STATUS_COLORS', () => {
    it('maps Success, Blocked, and Failed', () => {
      expect(STATUS_COLORS['Success']).toBeDefined()
      expect(STATUS_COLORS['Blocked']).toBeDefined()
      expect(STATUS_COLORS['Failed']).toBeDefined()
    })
  })

  describe('ACTION_COLORS', () => {
    it('maps all audit action types', () => {
      expect(ACTION_COLORS['Push']).toBeDefined()
      expect(ACTION_COLORS['BranchCreate']).toBeDefined()
      expect(ACTION_COLORS['Commit']).toBeDefined()
      expect(ACTION_COLORS['BlockedPush']).toBeDefined()
      expect(ACTION_COLORS['BlockedBranch']).toBeDefined()
      expect(ACTION_COLORS['StageFile']).toBeDefined()
    })
  })

  describe('FILE_STATUS_COLORS', () => {
    it('maps all git file statuses', () => {
      expect(FILE_STATUS_COLORS['M']).toBeDefined()
      expect(FILE_STATUS_COLORS['A']).toBeDefined()
      expect(FILE_STATUS_COLORS['D']).toBeDefined()
      expect(FILE_STATUS_COLORS['R']).toBeDefined()
      expect(FILE_STATUS_COLORS['?']).toBeDefined()
    })
  })

  describe('FILE_STATUS_LABELS', () => {
    it('has Spanish labels for all statuses', () => {
      expect(FILE_STATUS_LABELS['M']).toBe('Modificado')
      expect(FILE_STATUS_LABELS['A']).toBe('Agregado')
      expect(FILE_STATUS_LABELS['D']).toBe('Eliminado')
      expect(FILE_STATUS_LABELS['R']).toBe('Renombrado')
      expect(FILE_STATUS_LABELS['?']).toBe('Sin seguimiento')
    })
  })
})
