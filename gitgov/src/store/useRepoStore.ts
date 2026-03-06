import { create } from 'zustand'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import type {
  BranchInfo,
  BranchSyncStatus,
  FileChange,
  GitGovConfig,
  PendingPushPreview,
  RepoValidation,
} from '@/lib/types'

interface RepoState {
  repoPath: string | null
  config: GitGovConfig | null
  validation: RepoValidation | null
  currentBranch: string | null
  branchSync: BranchSyncStatus | null
  pendingPushPreview: PendingPushPreview | null
  branches: BranchInfo[]
  fileChanges: FileChange[]
  selectedFiles: Set<string>
  stagedFiles: Set<string>
  isLoadingStatus: boolean
  activeDiffFile: string | null
  activeDiff: string | null
  error: string | null
}

interface RepoActions {
  setRepoPath: (path: string) => Promise<void>
  validateRepo: (path: string) => Promise<RepoValidation>
  loadConfig: () => Promise<void>
  refreshStatus: () => Promise<void>
  refreshBranches: () => Promise<void>
  refreshBranchSync: (branch?: string) => Promise<BranchSyncStatus | null>
  refreshPendingPushPreview: (branch?: string) => Promise<PendingPushPreview | null>
  selectFile: (path: string) => void
  deselectFile: (path: string) => void
  selectAll: () => void
  deselectAll: () => void
  stageFiles: (paths: string[], developerLogin: string) => Promise<void>
  stageSelected: (developerLogin: string) => Promise<void>
  stageAllUnstaged: (developerLogin: string) => Promise<void>
  unstageAll: () => Promise<void>
  unstageFiles: (paths: string[]) => Promise<void>
  loadDiff: (filePath: string) => Promise<void>
  createBranch: (name: string, from: string, developerLogin: string, isAdmin: boolean, group?: string) => Promise<void>
  checkoutBranch: (name: string) => Promise<void>
  commit: (message: string, authorName: string, authorEmail: string, developerLogin: string) => Promise<string>
  push: (branch: string, developerLogin: string) => Promise<void>
  clearError: () => void
  reset: () => void
}

const initialState: RepoState = {
  repoPath: null,
  config: null,
  validation: null,
  currentBranch: null,
  branchSync: null,
  pendingPushPreview: null,
  branches: [],
  fileChanges: [],
  selectedFiles: new Set(),
  stagedFiles: new Set(),
  isLoadingStatus: false,
  activeDiffFile: null,
  activeDiff: null,
  error: null,
}

export const useRepoStore = create<RepoState & RepoActions>((set, get) => ({
  ...initialState,

  setRepoPath: async (path: string) => {
    set({ repoPath: path, isLoadingStatus: true })
    try {
      const validation = await get().validateRepo(path)
      set({ validation })
      if (validation.has_gitgov_toml) {
        await get().loadConfig()
      }
      await get().refreshStatus()
      await get().refreshBranches()
      await get().refreshBranchSync()
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    } finally {
      set({ isLoadingStatus: false })
    }
  },

  validateRepo: async (path: string) => {
    return tauriInvoke<RepoValidation>('cmd_validate_repo', { repoPath: path })
  },

  loadConfig: async () => {
    const { repoPath } = get()
    if (!repoPath) return
    try {
      const config = await tauriInvoke<GitGovConfig>('cmd_load_repo_config', { repoPath })
      set({ config })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  refreshStatus: async () => {
    const { repoPath } = get()
    if (!repoPath) return
    set({ isLoadingStatus: true })
    try {
      const changes = await tauriInvoke<FileChange[]>('cmd_get_status', { repoPath })
      const staged = new Set(changes.filter((c) => c.staged).map((c) => c.path))
      set({ fileChanges: changes, stagedFiles: staged })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    } finally {
      set({ isLoadingStatus: false })
    }
  },

  refreshBranches: async () => {
    const { repoPath } = get()
    if (!repoPath) return
    try {
      const branches = await tauriInvoke<BranchInfo[]>('cmd_list_branches', { repoPath })
      const current = branches.find((b) => b.is_current)
      set({ branches, currentBranch: current?.name ?? null })
      await get().refreshBranchSync(current?.name)
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  refreshBranchSync: async (branch?: string) => {
    const { repoPath, currentBranch } = get()
    if (!repoPath) {
      set({ branchSync: null, pendingPushPreview: null })
      return null
    }

    try {
      const resolvedBranch = branch ?? currentBranch ?? undefined
      const status = await tauriInvoke<BranchSyncStatus>('cmd_get_branch_sync_status', {
        repoPath,
        branch: resolvedBranch,
      })
      set({ branchSync: status })
      const pendingCommits = status.pending_local_commits ?? status.ahead
      if (pendingCommits > 0) {
        await get().refreshPendingPushPreview(status.branch)
      } else {
        set({ pendingPushPreview: null })
      }
      return status
    } catch (e) {
      set({
        error: parseCommandError(String(e)).message,
        branchSync: null,
        pendingPushPreview: null,
      })
      return null
    }
  },

  refreshPendingPushPreview: async (branch?: string) => {
    const { repoPath, currentBranch } = get()
    if (!repoPath) {
      set({ pendingPushPreview: null })
      return null
    }
    try {
      const resolvedBranch = branch ?? currentBranch ?? undefined
      const preview = await tauriInvoke<PendingPushPreview>('cmd_get_pending_push_preview', {
        repoPath,
        branch: resolvedBranch,
      })
      set({ pendingPushPreview: preview })
      return preview
    } catch (e) {
      set({ error: parseCommandError(String(e)).message, pendingPushPreview: null })
      return null
    }
  },

  selectFile: (path: string) => {
    const { selectedFiles } = get()
    const newSet = new Set(selectedFiles)
    newSet.add(path)
    set({ selectedFiles: newSet })
  },

  deselectFile: (path: string) => {
    const { selectedFiles } = get()
    const newSet = new Set(selectedFiles)
    newSet.delete(path)
    set({ selectedFiles: newSet })
  },

  selectAll: () => {
    const { fileChanges } = get()
    set({ selectedFiles: new Set(fileChanges.map((f) => f.path)) })
  },

  deselectAll: () => {
    set({ selectedFiles: new Set() })
  },

  stageFiles: async (paths: string[], developerLogin: string) => {
    const { repoPath } = get()
    if (!repoPath || paths.length === 0) return
    try {
      await tauriInvoke('cmd_stage_files', {
        repoPath,
        files: paths,
        developerLogin,
      })
      await get().refreshStatus()
      set({ selectedFiles: new Set() })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  stageSelected: async (developerLogin: string) => {
    const { selectedFiles } = get()
    await get().stageFiles(Array.from(selectedFiles), developerLogin)
  },

  stageAllUnstaged: async (developerLogin: string) => {
    const { repoPath, fileChanges } = get()
    if (!repoPath) return

    const unstagedPaths = fileChanges.filter((f) => !f.staged).map((f) => f.path)
    if (unstagedPaths.length === 0) return

    await get().stageFiles(unstagedPaths, developerLogin)
  },

  unstageAll: async () => {
    const { repoPath } = get()
    if (!repoPath) return
    try {
      await tauriInvoke('cmd_unstage_all', { repoPath })
      await get().refreshStatus()
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  unstageFiles: async (paths: string[]) => {
    const { repoPath } = get()
    if (!repoPath || paths.length === 0) return
    try {
      await tauriInvoke('cmd_unstage_files', { repoPath, files: paths })
      await get().refreshStatus()
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
    }
  },

  loadDiff: async (filePath: string) => {
    const { repoPath } = get()
    if (!repoPath) return
    set({ activeDiffFile: filePath, activeDiff: null })
    try {
      const diff = await tauriInvoke<string>('cmd_get_file_diff', { repoPath, filePath })
      set({ activeDiff: diff })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message, activeDiff: null })
    }
  },

  createBranch: async (name: string, from: string, developerLogin: string, isAdmin: boolean, group?: string) => {
    const { repoPath } = get()
    if (!repoPath) return
    try {
      await tauriInvoke('cmd_create_branch', {
        repoPath,
        name,
        fromBranch: from,
        actor: {
          developerLogin,
          isAdmin,
          userGroup: group ?? null,
        },
      })
      await get().refreshBranches()
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      throw e
    }
  },

  checkoutBranch: async (name: string) => {
    const { repoPath } = get()
    if (!repoPath) return
    try {
      await tauriInvoke('cmd_checkout_branch', { repoPath, name })
      await get().refreshBranches()
      await get().refreshStatus()
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      throw e
    }
  },

  commit: async (message: string, authorName: string, authorEmail: string, developerLogin: string) => {
    const { repoPath } = get()
    if (!repoPath) throw new Error('Ningún repositorio seleccionado')
    try {
      const hash = await tauriInvoke<string>('cmd_commit', {
        repoPath,
        message,
        authorName,
        authorEmail,
        developerLogin,
      })
      await get().refreshStatus()
      await get().refreshBranchSync()
      return hash
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      throw e
    }
  },

  push: async (branch: string, developerLogin: string) => {
    const { repoPath } = get()
    if (!repoPath) throw new Error('Ningún repositorio seleccionado')
    try {
      await tauriInvoke('cmd_push', { repoPath, branch, developerLogin })
    } catch (e) {
      set({ error: parseCommandError(String(e)).message })
      throw e
    } finally {
      await get().refreshBranchSync(branch)
    }
  },

  clearError: () => set({ error: null }),

  reset: () => set(initialState),
}))
