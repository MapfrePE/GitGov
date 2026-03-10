import { describe, it, expect, beforeEach, vi } from 'vitest'

// Mock Tauri APIs before importing the store
const mockInvoke = vi.fn()
const mockListen = vi.fn().mockResolvedValue(() => {})

vi.mock('@/lib/tauri', () => ({
  tauriInvoke: (...args: unknown[]) => mockInvoke(...args),
  tauriListen: (...args: unknown[]) => mockListen(...args),
  parseCommandError: (error: string) => {
    try {
      const parsed = JSON.parse(error)
      return { code: parsed.code || 'UNKNOWN', message: parsed.message || error }
    } catch {
      return { code: 'UNKNOWN', message: error }
    }
  },
}))

const mockNotifyBlocked = vi.fn()
const mockNotifyGovWarn = vi.fn()

vi.mock('@/lib/notifications', () => ({
  notifyBlockedPush: (...args: unknown[]) => mockNotifyBlocked(...args),
  notifyGovernanceWarning: (...args: unknown[]) => mockNotifyGovWarn(...args),
}))

import { useRepoStore } from '@/store/useRepoStore'

describe('useRepoStore', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useRepoStore.getState().reset()
  })

  describe('file selection', () => {
    it('selectFile adds a file to selectedFiles', () => {
      useRepoStore.getState().selectFile('src/main.ts')
      expect(useRepoStore.getState().selectedFiles.has('src/main.ts')).toBe(true)
    })

    it('deselectFile removes a file from selectedFiles', () => {
      useRepoStore.getState().selectFile('src/main.ts')
      useRepoStore.getState().selectFile('src/app.ts')
      useRepoStore.getState().deselectFile('src/main.ts')
      expect(useRepoStore.getState().selectedFiles.has('src/main.ts')).toBe(false)
      expect(useRepoStore.getState().selectedFiles.has('src/app.ts')).toBe(true)
    })

    it('selectAll selects all fileChanges', () => {
      // Manually set fileChanges (simulate loaded state)
      useRepoStore.setState({
        fileChanges: [
          { path: 'a.ts', status: 'Modified', staged: false },
          { path: 'b.ts', status: 'Added', staged: false },
        ],
      })
      useRepoStore.getState().selectAll()
      const selected = useRepoStore.getState().selectedFiles
      expect(selected.size).toBe(2)
      expect(selected.has('a.ts')).toBe(true)
      expect(selected.has('b.ts')).toBe(true)
    })

    it('deselectAll clears selectedFiles', () => {
      useRepoStore.getState().selectFile('a.ts')
      useRepoStore.getState().selectFile('b.ts')
      useRepoStore.getState().deselectAll()
      expect(useRepoStore.getState().selectedFiles.size).toBe(0)
    })
  })

  describe('clearError', () => {
    it('sets error to null', () => {
      useRepoStore.setState({ error: 'some error' })
      useRepoStore.getState().clearError()
      expect(useRepoStore.getState().error).toBeNull()
    })
  })

  describe('reset', () => {
    it('resets all state to initial values', () => {
      useRepoStore.setState({
        repoPath: '/some/path',
        error: 'error',
        currentBranch: 'main',
      })
      useRepoStore.getState().reset()
      expect(useRepoStore.getState().repoPath).toBeNull()
      expect(useRepoStore.getState().error).toBeNull()
      expect(useRepoStore.getState().currentBranch).toBeNull()
    })
  })

  describe('repo switch UX', () => {
    it('beginRepoSwitch preserves previous repo and clears active repo', () => {
      useRepoStore.setState({
        repoPath: '/repo/actual',
        currentBranch: 'main',
        fileChanges: [{ path: 'a.ts', status: 'Modified', staged: false }],
      })

      useRepoStore.getState().beginRepoSwitch()
      const state = useRepoStore.getState()
      expect(state.previousRepoPath).toBe('/repo/actual')
      expect(state.repoPath).toBeNull()
      expect(state.currentBranch).toBeNull()
      expect(state.fileChanges).toEqual([])
    })

    it('cancelRepoSwitch restores previous repo', async () => {
      useRepoStore.setState({
        repoPath: null,
        previousRepoPath: '/repo/anterior',
      })

      mockInvoke
        .mockResolvedValueOnce({
          path_exists: true,
          is_git_repo: true,
          has_remote_origin: true,
          has_gitgov_toml: false,
        }) // cmd_validate_repo
        .mockResolvedValueOnce([]) // cmd_get_status
        .mockResolvedValueOnce([]) // cmd_list_branches
        .mockResolvedValueOnce({
          branch: 'main',
          upstream: null,
          has_upstream: false,
          ahead: 0,
          behind: 0,
          pending_local_commits: 0,
        }) // cmd_get_branch_sync_status (from refreshBranches)
        .mockResolvedValueOnce({
          branch: 'main',
          upstream: null,
          has_upstream: false,
          ahead: 0,
          behind: 0,
          pending_local_commits: 0,
        }) // cmd_get_branch_sync_status (explicit)

      await useRepoStore.getState().cancelRepoSwitch()
      const state = useRepoStore.getState()
      expect(state.repoPath).toBe('/repo/anterior')
      expect(state.previousRepoPath).toBeNull()
      expect(mockInvoke).toHaveBeenCalledWith('cmd_validate_repo', { repoPath: '/repo/anterior' })
    })
  })

  describe('push', () => {
    it('throws when no repo is selected', async () => {
      await expect(
        useRepoStore.getState().push('main', 'dev1'),
      ).rejects.toThrow('Ningún repositorio seleccionado')
    })

    it('calls cmd_push with correct args on success', async () => {
      useRepoStore.setState({ repoPath: '/repo' })
      mockInvoke.mockResolvedValueOnce(undefined) // cmd_push succeeds
      mockInvoke.mockResolvedValueOnce({ ahead: 0, behind: 0, branch: 'main' }) // refreshBranchSync

      await useRepoStore.getState().push('main', 'dev1')

      // tauriListen should be called to listen for governance warnings
      expect(mockListen).toHaveBeenCalledWith(
        'gitgov:governance-warnings',
        expect.any(Function),
      )

      // cmd_push invoked with correct params
      expect(mockInvoke).toHaveBeenCalledWith('cmd_push', {
        repoPath: '/repo',
        branch: 'main',
        developerLogin: 'dev1',
      })
    })

    it('sets error and notifies on BLOCKED push', async () => {
      useRepoStore.setState({ repoPath: '/repo' })
      const blockedError = JSON.stringify({
        code: 'BLOCKED',
        message: 'Branch main está protegida',
      })
      mockInvoke.mockRejectedValueOnce(blockedError)
      mockInvoke.mockResolvedValueOnce({ ahead: 0, behind: 0, branch: 'main' }) // refreshBranchSync

      await expect(
        useRepoStore.getState().push('main', 'dev1'),
      ).rejects.toBeDefined()

      expect(useRepoStore.getState().error).toBe('Branch main está protegida')
      expect(mockNotifyBlocked).toHaveBeenCalledWith('main', 'Branch main está protegida')
    })

    it('sets error and notifies on GOVERNANCE_BLOCKED push', async () => {
      useRepoStore.setState({ repoPath: '/repo' })
      const blockedError = JSON.stringify({
        code: 'GOVERNANCE_BLOCKED',
        message: 'Requires pull request',
      })
      mockInvoke.mockRejectedValueOnce(blockedError)
      mockInvoke.mockResolvedValueOnce({ ahead: 0, behind: 0, branch: 'main' })

      await expect(
        useRepoStore.getState().push('main', 'dev1'),
      ).rejects.toBeDefined()

      expect(mockNotifyBlocked).toHaveBeenCalledWith('main', 'Requires pull request')
    })

    it('cleans up listener even when push fails', async () => {
      useRepoStore.setState({ repoPath: '/repo' })
      const mockUnlisten = vi.fn()
      mockListen.mockResolvedValueOnce(mockUnlisten)
      mockInvoke.mockRejectedValueOnce('network error')
      mockInvoke.mockResolvedValueOnce({ ahead: 0, behind: 0, branch: 'main' })

      await expect(
        useRepoStore.getState().push('main', 'dev1'),
      ).rejects.toBeDefined()

      // unlisten must be called in finally
      expect(mockUnlisten).toHaveBeenCalled()
    })

    it('cleans up listener on successful push', async () => {
      useRepoStore.setState({ repoPath: '/repo' })
      const mockUnlisten = vi.fn()
      mockListen.mockResolvedValueOnce(mockUnlisten)
      mockInvoke.mockResolvedValueOnce(undefined) // cmd_push
      mockInvoke.mockResolvedValueOnce({ ahead: 0, behind: 0, branch: 'main' })

      await useRepoStore.getState().push('main', 'dev1')

      expect(mockUnlisten).toHaveBeenCalled()
    })

    it('does not notify for non-blocked errors', async () => {
      useRepoStore.setState({ repoPath: '/repo' })
      const genericError = JSON.stringify({
        code: 'NETWORK',
        message: 'Connection refused',
      })
      mockInvoke.mockRejectedValueOnce(genericError)
      mockInvoke.mockResolvedValueOnce({ ahead: 0, behind: 0, branch: 'main' })

      await expect(
        useRepoStore.getState().push('feat', 'dev1'),
      ).rejects.toBeDefined()

      expect(mockNotifyBlocked).not.toHaveBeenCalled()
      expect(useRepoStore.getState().error).toBe('Connection refused')
    })
  })

  describe('commit', () => {
    it('throws when no repo is selected', async () => {
      await expect(
        useRepoStore.getState().commit('msg', 'Name', 'email', 'dev1'),
      ).rejects.toThrow('Ningún repositorio seleccionado')
    })

    it('returns commit hash on success', async () => {
      useRepoStore.setState({ repoPath: '/repo' })
      mockInvoke
        .mockResolvedValueOnce('abc123') // cmd_commit
        .mockResolvedValueOnce([]) // refreshStatus → cmd_get_status
        .mockResolvedValueOnce({ ahead: 1, behind: 0, branch: 'main' }) // refreshBranchSync
        .mockResolvedValueOnce({ commits: [] }) // refreshPendingPushPreview

      const hash = await useRepoStore.getState().commit('fix: bug', 'Dev', 'dev@test.com', 'dev1')
      expect(hash).toBe('abc123')
      expect(mockInvoke).toHaveBeenCalledWith('cmd_commit', {
        repoPath: '/repo',
        message: 'fix: bug',
        authorName: 'Dev',
        authorEmail: 'dev@test.com',
        developerLogin: 'dev1',
      })
    })
  })
})
