import { memo, useEffect, useMemo, useRef, useState } from 'react'
import clsx from 'clsx'
import { useRepoStore } from '@/store/useRepoStore'
import { useAuthStore } from '@/store/useAuthStore'
import type { FileChange } from '@/lib/types'
import { FILE_STATUS_COLORS } from '@/lib/constants'
import {
  analyzeLargeChangeset,
  firstMatchingGitGovIgnoreRule,
  isHiddenByGitGovIgnore,
  isLikelyGeneratedNoisePath,
  parseGitGovIgnoreRules,
  topLevelFolder,
} from '@/lib/largeChangeset'
import { tauriInvoke, parseCommandError } from '@/lib/tauri'
import { FileText, AlertCircle, CheckSquare, Plus, FileCode, Loader2, Search, Sparkles, FolderTree, ChevronRight, ChevronDown } from 'lucide-react'
import { SkeletonFileRow } from '@/components/shared/Skeleton'

const LARGE_CHANGESET_THRESHOLD = 1000
const INITIAL_VISIBLE_ROWS = 300
const VISIBLE_ROWS_STEP = 700
const FLAT_LIST_VIRTUALIZE_THRESHOLD = 250
const VIRTUAL_ROW_HEIGHT = 36
const VIRTUAL_OVERSCAN_ROWS = 10

interface IgnoreRuleApplyResult {
  target: string
  target_path: string
  rules_requested: number
  rules_added: number
  file_existed: boolean
}

interface GitGovIgnoreReadResult {
  exists: boolean
  path: string
  content: string
}

interface IgnoreRuleRemoveResult {
  target: string
  target_path: string
  rules_requested: number
  rules_removed: number
  file_existed: boolean
}

interface FileItemProps {
  file: FileChange
  selected: boolean
  disabled: boolean
  gitgovHiddenRule?: string | null
  showGitGovHiddenBadge?: boolean
  onToggle: () => void
  onViewDiff: () => void
  onUnstage: () => void
}

interface GroupedFileBucket {
  folder: string
  files: FileChange[]
}

type FileNoiseFilterMode = 'all' | 'code' | 'noise'

interface FileListUiPrefs {
  noiseFilterMode?: FileNoiseFilterMode
  selectedStackTemplateId?: string | null
  selectedStackRuleGroupId?: string | null
  showGitGovHidden?: boolean
}

function fileListPrefsKey(repoPath: string): string {
  return `gitgov:filelist:prefs:${encodeURIComponent(repoPath)}`
}

function readFileListPrefs(repoPath: string): FileListUiPrefs | null {
  try {
    const raw = window.localStorage.getItem(fileListPrefsKey(repoPath))
    if (!raw) return null
    const parsed = JSON.parse(raw) as FileListUiPrefs
    return parsed && typeof parsed === 'object' ? parsed : null
  } catch {
    return null
  }
}

function writeFileListPrefs(repoPath: string, prefs: FileListUiPrefs): void {
  try {
    window.localStorage.setItem(fileListPrefsKey(repoPath), JSON.stringify(prefs))
  } catch {
    // no-op: localStorage puede estar bloqueado/lleno
  }
}

const FileItem = memo(function FileItem({
  file,
  selected,
  disabled,
  gitgovHiddenRule,
  showGitGovHiddenBadge,
  onToggle,
  onViewDiff,
  onUnstage,
}: FileItemProps) {
  const statusChar = {
    Modified: 'M',
    Added: 'A',
    Deleted: 'D',
    Renamed: 'R',
    Untracked: '?',
  }[file.status]
  const interactionsBlocked = disabled

  const lastSlash = file.path.lastIndexOf('/')
  const dir = lastSlash >= 0 ? file.path.slice(0, lastSlash) : ''
  const name = lastSlash >= 0 ? file.path.slice(lastSlash + 1) : file.path

  return (
    <div
      className={clsx(
        'flex items-center gap-2.5 px-3 py-2 cursor-pointer group transition-colors duration-150',
        selected ? 'bg-white/3' : 'hover:bg-white/2',
        disabled && 'opacity-50'
      )}
    >
      <button
        onClick={onToggle}
        disabled={interactionsBlocked}
        className="shrink-0"
      >
        <CheckSquare
          size={14}
          strokeWidth={1.5}
          className={clsx(
            'transition-colors',
            selected ? 'text-brand-400' : 'text-surface-600',
            'hover:text-brand-400'
          )}
        />
      </button>

        <span
          className={clsx(
            'shrink-0 w-4 h-4 rounded text-[9px] font-semibold mono-data flex items-center justify-center',
            FILE_STATUS_COLORS[statusChar ?? '?']
          )}
        >
          {statusChar}
        </span>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1">
          {dir ? <span className="text-[11px] text-surface-600 truncate">{dir}/</span> : null}
          <span className="text-xs text-surface-200 truncate font-medium">{name}</span>
          {showGitGovHiddenBadge && gitgovHiddenRule && (
            <span
              title={`Oculto por .gitgovignore (${gitgovHiddenRule})`}
              className="text-[9px] px-1.5 py-0.5 rounded bg-warning-500/12 text-warning-300 border border-warning-500/20 shrink-0"
            >
              GitGov-hidden
            </span>
          )}
        </div>
      </div>

      {file.staged && (
        <button
          onClick={(e) => { e.stopPropagation(); onUnstage() }}
          title="Quitar del staging"
          className="text-[9px] font-medium bg-brand-500/10 text-brand-400 px-1.5 py-0.5 rounded hover:bg-danger-500/15 hover:text-danger-400 transition-colors"
        >
          Staged ×
        </button>
      )}

      {disabled && (
        <div className="relative group">
          <AlertCircle size={12} strokeWidth={1.5} className="text-warning-500" />
          <span className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-2 py-1 bg-surface-900 text-[10px] text-white rounded opacity-0 group-hover:opacity-100 whitespace-nowrap">
            Path no permitido
          </span>
        </div>
      )}

      <button
        onClick={onViewDiff}
        className="opacity-0 group-hover:opacity-100 text-surface-500 hover:text-surface-300 transition-all duration-150"
      >
        <FileText size={13} strokeWidth={1.5} />
      </button>
    </div>
  )
})

export function FileList() {
  const {
    repoPath,
    fileChanges,
    pendingPushPreview,
    selectedFiles,
    stagedFiles,
    isLoadingStatus,
    selectFile,
    deselectFile,
    deselectAll,
    stageFiles,
    loadDiff,
    stageSelected,
    stageAllUnstaged,
    unstageFiles,
    refreshStatus,
  } = useRepoStore()

  const { user } = useAuthStore()
  const [isPreparing, setIsPreparing] = useState(false)
  const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE_ROWS)
  const [searchQuery, setSearchQuery] = useState('')
  const [noiseFilterMode, setNoiseFilterMode] = useState<FileNoiseFilterMode>('all')
  const [showIgnorePreview, setShowIgnorePreview] = useState(false)
  const [isApplyingIgnoreRules, setIsApplyingIgnoreRules] = useState(false)
  const [isMovingGitGovRule, setIsMovingGitGovRule] = useState<string | null>(null)
  const [ignoreRulesError, setIgnoreRulesError] = useState<string | null>(null)
  const [ignoreRulesSuccess, setIgnoreRulesSuccess] = useState<string | null>(null)
  const [lastHiddenByRules, setLastHiddenByRules] = useState<number | null>(null)
  const [gitgovIgnoreRules, setGitgovIgnoreRules] = useState<string[]>([])
  const [gitgovIgnoreExists, setGitgovIgnoreExists] = useState(false)
  const [gitgovIgnorePath, setGitgovIgnorePath] = useState<string | null>(null)
  const [gitgovIgnoreLoadError, setGitgovIgnoreLoadError] = useState<string | null>(null)
  const [showGitGovHidden, setShowGitGovHidden] = useState(false)
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set())
  const [selectedStackTemplateId, setSelectedStackTemplateId] = useState<string | null>(null)
  const [selectedStackRuleGroupId, setSelectedStackRuleGroupId] = useState<string | null>(null)
  const [listScrollTop, setListScrollTop] = useState(0)
  const [listViewportHeight, setListViewportHeight] = useState(480)
  const [busyGroupActionKey, setBusyGroupActionKey] = useState<string | null>(null)
  const listContainerRef = useRef<HTMLDivElement | null>(null)
  const pendingPushFilesCountRaw = pendingPushPreview?.files.length ?? 0

  useEffect(() => {
    setVisibleCount(INITIAL_VISIBLE_ROWS)
  }, [fileChanges.length, pendingPushFilesCountRaw, searchQuery])

  useEffect(() => {
    if (!repoPath) return
    const saved = readFileListPrefs(repoPath)
    if (!saved) {
      setNoiseFilterMode('all')
      setSelectedStackTemplateId(null)
      setSelectedStackRuleGroupId(null)
      return
    }

    if (saved.noiseFilterMode === 'all' || saved.noiseFilterMode === 'code' || saved.noiseFilterMode === 'noise') {
      setNoiseFilterMode(saved.noiseFilterMode)
    } else {
      setNoiseFilterMode('all')
    }
    setSelectedStackTemplateId(saved.selectedStackTemplateId ?? null)
    setSelectedStackRuleGroupId(saved.selectedStackRuleGroupId ?? null)
    setShowGitGovHidden(saved.showGitGovHidden === true)
  }, [repoPath])

  useEffect(() => {
    let cancelled = false

    if (!repoPath) {
      setGitgovIgnoreRules([])
      setGitgovIgnoreExists(false)
      setGitgovIgnorePath(null)
      setGitgovIgnoreLoadError(null)
      return
    }

    const loadGitGovIgnore = async () => {
      try {
        const res = await tauriInvoke<GitGovIgnoreReadResult>('cmd_read_gitgovignore', { repoPath })
        if (cancelled) return
        setGitgovIgnoreExists(res.exists)
        setGitgovIgnorePath(res.path)
        setGitgovIgnoreRules(parseGitGovIgnoreRules(res.content))
        setGitgovIgnoreLoadError(null)
      } catch (e) {
        if (cancelled) return
        setGitgovIgnoreLoadError(parseCommandError(String(e)).message)
        setGitgovIgnoreRules([])
      }
    }

    void loadGitGovIgnore()
    return () => {
      cancelled = true
    }
  }, [repoPath])

  useEffect(() => {
    const node = listContainerRef.current
    if (!node) return

    const measure = () => setListViewportHeight(node.clientHeight || 480)
    measure()

    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(measure)
    observer.observe(node)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    setListScrollTop(0)
    const node = listContainerRef.current
    if (node) {
      node.scrollTop = 0
    }
  }, [repoPath, searchQuery, fileChanges.length, pendingPushFilesCountRaw])

  const handleToggle = (path: string, isSelected: boolean) => {
    if (isSelected) {
      deselectFile(path)
    } else {
      selectFile(path)
    }
  }

  const handleSelectAllVisible = () => {
    deselectAll()
    for (const file of visibleFiles) {
      selectFile(file.path)
    }
  }

  const handleStageSelected = async () => {
    if (selectedFiles.size > 0 && user) {
      await stageSelected(user.login)
    }
  }

  const handlePrepareAll = async () => {
    if (!user) return
    setIsPreparing(true)
    try {
      await stageAllUnstaged(user.login)
    } finally {
      setIsPreparing(false)
    }
  }

  const pendingPushFiles = pendingPushPreview?.files
  const workingTreePathSet = useMemo(() => new Set(fileChanges.map((f) => f.path)), [fileChanges])
  const pendingPushOnlyFiles = useMemo<FileChange[]>(
    () =>
      (pendingPushFiles ?? [])
        .filter((entry) => !workingTreePathSet.has(entry.path))
        .map((entry) => ({
          path: entry.path,
          status: 'Modified',
          staged: false,
        })),
    [pendingPushFiles, workingTreePathSet]
  )
  const effectiveFileChanges = useMemo(
    () => [...pendingPushOnlyFiles, ...fileChanges],
    [pendingPushOnlyFiles, fileChanges]
  )

  const analysis = useMemo(() => analyzeLargeChangeset(effectiveFileChanges), [effectiveFileChanges])
  const normalizedQuery = searchQuery.trim().toLowerCase()
  const gitgovHiddenPathSet = useMemo(() => {
    if (gitgovIgnoreRules.length === 0) return new Set<string>()
    const set = new Set<string>()
    for (const f of effectiveFileChanges) {
      if (isHiddenByGitGovIgnore(f.path, gitgovIgnoreRules)) set.add(f.path)
    }
    return set
  }, [effectiveFileChanges, gitgovIgnoreRules])
  const gitgovHiddenCount = gitgovHiddenPathSet.size
  const gitgovHiddenRuleByPath = useMemo(() => {
    const map = new Map<string, string>()
    if (gitgovIgnoreRules.length === 0) return map
    for (const f of effectiveFileChanges) {
      const matched = firstMatchingGitGovIgnoreRule(f.path, gitgovIgnoreRules)
      if (matched) {
        map.set(f.path, matched)
      }
    }
    return map
  }, [effectiveFileChanges, gitgovIgnoreRules])
  const gitgovHiddenTopRules = useMemo(() => {
    if (gitgovIgnoreRules.length === 0 || gitgovHiddenCount === 0) return [] as Array<{ rule: string; count: number }>
    const counts = new Map<string, number>()
    for (const [, matched] of gitgovHiddenRuleByPath) {
      counts.set(matched, (counts.get(matched) ?? 0) + 1)
    }
    return Array.from(counts.entries())
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .slice(0, 6)
      .map(([rule, count]) => ({ rule, count }))
  }, [gitgovHiddenCount, gitgovHiddenRuleByPath, gitgovIgnoreRules])
  const gitgovVisibleFiles = useMemo(() => {
    if (showGitGovHidden || gitgovHiddenCount === 0) return effectiveFileChanges
    return effectiveFileChanges.filter((f) => !gitgovHiddenPathSet.has(f.path))
  }, [effectiveFileChanges, showGitGovHidden, gitgovHiddenCount, gitgovHiddenPathSet])
  const noisePathSet = useMemo(() => {
    const set = new Set<string>()
    for (const f of gitgovVisibleFiles) {
      if (isLikelyGeneratedNoisePath(f.path)) set.add(f.path)
    }
    return set
  }, [gitgovVisibleFiles])
  const noiseFilesCount = noisePathSet.size
  const codeFilesCount = Math.max(0, gitgovVisibleFiles.length - noiseFilesCount)

  const noiseFilteredFiles = useMemo(() => {
    if (noiseFilterMode === 'all') return gitgovVisibleFiles
    if (noiseFilterMode === 'noise') return gitgovVisibleFiles.filter((f) => noisePathSet.has(f.path))
    return gitgovVisibleFiles.filter((f) => !noisePathSet.has(f.path))
  }, [gitgovVisibleFiles, noiseFilterMode, noisePathSet])

  const filteredFiles = useMemo(() => {
    if (!normalizedQuery) return noiseFilteredFiles
    return noiseFilteredFiles.filter((f) => f.path.toLowerCase().includes(normalizedQuery))
  }, [noiseFilteredFiles, normalizedQuery])

  useEffect(() => {
    if (searchQuery.trim()) return
    if (effectiveFileChanges.length <= LARGE_CHANGESET_THRESHOLD) {
      setExpandedGroups(new Set())
      return
    }

    const groups = Array.from(
      effectiveFileChanges.reduce((acc, file) => {
        const folder = topLevelFolder(file.path)
        acc.set(folder, (acc.get(folder) ?? 0) + 1)
        return acc
      }, new Map<string, number>()).entries()
    )
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .slice(0, 3)
      .map(([folder]) => folder)

    setExpandedGroups(new Set(groups))
  }, [effectiveFileChanges, searchQuery])

  useEffect(() => {
    if (analysis.stackTemplates.length === 0) {
      setSelectedStackTemplateId(null)
      setSelectedStackRuleGroupId(null)
      return
    }
    setSelectedStackTemplateId((prev) => (
      prev && analysis.stackTemplates.some((tpl) => tpl.id === prev)
        ? prev
        : analysis.stackTemplates[0].id
    ))
  }, [analysis.stackTemplates])

  const applyRulesWithConfirmation = async (
    target: 'gitignore' | 'exclude' | 'gitgovignore',
    rules: string[],
    sourceLabel: string,
  ) => {
    if (!repoPath || rules.length === 0) return

    const targetLabel =
      target === 'gitignore'
        ? '.gitignore'
        : target === 'gitgovignore'
          ? '.gitgovignore'
          : '.git/info/exclude'
    const confirmed = window.confirm(
      `Aplicar ${rules.length} regla(s) de ${sourceLabel} a ${targetLabel}?\n\n${
        target === 'gitgovignore'
          ? 'Esto ocultará archivos solo en GitGov (no altera Git).'
          : 'Esto ocultará archivos coincidentes en Git y GitGov.'
      }`
    )
    if (!confirmed) return

    const beforeCount = useRepoStore.getState().fileChanges.length
    setIsApplyingIgnoreRules(true)
    setIgnoreRulesError(null)
    setIgnoreRulesSuccess(null)

    try {
      const result = await tauriInvoke<IgnoreRuleApplyResult>('cmd_apply_ignore_rules', {
        repoPath,
        target,
        rules,
      })

      await refreshStatus()
      if (target === 'gitgovignore') {
        const gitgovRes = await tauriInvoke<GitGovIgnoreReadResult>('cmd_read_gitgovignore', { repoPath })
        setGitgovIgnoreExists(gitgovRes.exists)
        setGitgovIgnorePath(gitgovRes.path)
        setGitgovIgnoreRules(parseGitGovIgnoreRules(gitgovRes.content))
        setGitgovIgnoreLoadError(null)
      }
      const afterCount = useRepoStore.getState().fileChanges.length
      const hiddenDelta = Math.max(0, beforeCount - afterCount)
      setLastHiddenByRules(hiddenDelta)
      setIgnoreRulesSuccess(
        `${sourceLabel}: reglas añadidas a ${targetLabel}: ${result.rules_added}/${result.rules_requested}. Ocultados tras refrescar: ${hiddenDelta}.`
      )
      if (hiddenDelta > 0) {
        setShowIgnorePreview(false)
      }
    } catch (e) {
      setIgnoreRulesError(parseCommandError(String(e)).message)
    } finally {
      setIsApplyingIgnoreRules(false)
    }
  }

  const applyIgnoreRules = async (target: 'gitignore' | 'exclude') => {
    if (analysis.rules.length === 0) return
    await applyRulesWithConfirmation(target, analysis.rules.map((r) => r.rule), 'heurística de limpieza')
  }

  const selectedStackTemplate = analysis.stackTemplates.find((tpl) => tpl.id === selectedStackTemplateId) ?? null
  const selectedStackRuleGroup = selectedStackTemplate
    ? selectedStackTemplate.ruleGroups.find((g) => g.id === selectedStackRuleGroupId) ?? selectedStackTemplate.ruleGroups[0]
    : null

  useEffect(() => {
    if (!selectedStackTemplate) {
      setSelectedStackRuleGroupId(null)
      return
    }
    setSelectedStackRuleGroupId((prev) => (
      prev && selectedStackTemplate.ruleGroups.some((g) => g.id === prev)
        ? prev
        : selectedStackTemplate.ruleGroups[0]?.id ?? null
    ))
  }, [selectedStackTemplate])

  useEffect(() => {
    if (!repoPath) return
    writeFileListPrefs(repoPath, {
      noiseFilterMode,
      selectedStackTemplateId,
      selectedStackRuleGroupId,
      showGitGovHidden,
    })
  }, [repoPath, noiseFilterMode, selectedStackTemplateId, selectedStackRuleGroupId, showGitGovHidden])

  const applyStackTemplateRules = async (target: 'gitignore' | 'exclude' | 'gitgovignore') => {
    if (!selectedStackTemplate || !selectedStackRuleGroup) return
    await applyRulesWithConfirmation(
      target,
      selectedStackRuleGroup.rules,
      `pack ${selectedStackTemplate.label} / ${selectedStackRuleGroup.label}`
    )
  }

  const reloadGitGovIgnoreRules = async () => {
    if (!repoPath) return
    const gitgovRes = await tauriInvoke<GitGovIgnoreReadResult>('cmd_read_gitgovignore', { repoPath })
    setGitgovIgnoreExists(gitgovRes.exists)
    setGitgovIgnorePath(gitgovRes.path)
    setGitgovIgnoreRules(parseGitGovIgnoreRules(gitgovRes.content))
    setGitgovIgnoreLoadError(null)
  }

  const moveGitGovRuleToTarget = async (rule: string, target: 'gitignore' | 'exclude') => {
    if (!repoPath) return
    const targetLabel = target === 'gitignore' ? '.gitignore' : '.git/info/exclude'
    const confirmed = window.confirm(
      `Mover la regla "${rule}" de .gitgovignore a ${targetLabel}?\n\nSe añadirá a ${targetLabel} y se quitará de .gitgovignore.`
    )
    if (!confirmed) return

    setIsMovingGitGovRule(rule)
    setIgnoreRulesError(null)
    setIgnoreRulesSuccess(null)
    try {
      await tauriInvoke<IgnoreRuleApplyResult>('cmd_apply_ignore_rules', {
        repoPath,
        target,
        rules: [rule],
      })
      const removeResult = await tauriInvoke<IgnoreRuleRemoveResult>('cmd_remove_ignore_rules', {
        repoPath,
        target: 'gitgovignore',
        rules: [rule],
      })
      await refreshStatus()
      await reloadGitGovIgnoreRules()
      setIgnoreRulesSuccess(`Regla movida a ${targetLabel}. Eliminada de .gitgovignore: ${removeResult.rules_removed > 0 ? 'sí' : 'no'}.`)
    } catch (e) {
      setIgnoreRulesError(parseCommandError(String(e)).message)
    } finally {
      setIsMovingGitGovRule(null)
    }
  }

  const exportGitGovIgnorePreset = () => {
    if (!selectedStackTemplate || !selectedStackRuleGroup) return
    const lines = [
      '# GitGov preset (.gitgovignore)',
      `# Stack: ${selectedStackTemplate.label}`,
      `# Sub-pack: ${selectedStackRuleGroup.label}`,
      '# Nota: oculta solo en GitGov (no altera Git)',
      '',
      ...selectedStackRuleGroup.rules,
      '',
    ]
    const blob = new Blob([lines.join('\n')], { type: 'text/plain;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `.gitgovignore.${selectedStackTemplate.id}.${selectedStackRuleGroup.id}.txt`
    document.body.appendChild(a)
    a.click()
    a.remove()
    URL.revokeObjectURL(url)
  }

  const unstagedCount = useMemo(() => fileChanges.reduce((acc, f) => acc + (f.staged ? 0 : 1), 0), [fileChanges])
  const hasUnstagedFiles = unstagedCount > 0
  const someSelected = selectedFiles.size > 0
  const isLargeChangeset = effectiveFileChanges.length > LARGE_CHANGESET_THRESHOLD
  const visibleFiles = isLargeChangeset ? filteredFiles.slice(0, visibleCount) : filteredFiles
  const hiddenFilesCount = filteredFiles.length - visibleFiles.length
  const shouldVirtualizeFlatList = !isLargeChangeset && filteredFiles.length > FLAT_LIST_VIRTUALIZE_THRESHOLD

  const flatVirtualWindow = useMemo(() => {
    if (!shouldVirtualizeFlatList) {
      return {
        startIndex: 0,
        endIndex: filteredFiles.length,
        topPadding: 0,
        bottomPadding: 0,
        files: filteredFiles,
      }
    }

    const viewportRows = Math.ceil(listViewportHeight / VIRTUAL_ROW_HEIGHT)
    const rawStart = Math.floor(listScrollTop / VIRTUAL_ROW_HEIGHT) - VIRTUAL_OVERSCAN_ROWS
    const startIndex = Math.max(0, rawStart)
    const endIndex = Math.min(
      filteredFiles.length,
      startIndex + viewportRows + VIRTUAL_OVERSCAN_ROWS * 2
    )
    const topPadding = startIndex * VIRTUAL_ROW_HEIGHT
    const bottomPadding = Math.max(0, (filteredFiles.length - endIndex) * VIRTUAL_ROW_HEIGHT)

    return {
      startIndex,
      endIndex,
      topPadding,
      bottomPadding,
      files: filteredFiles.slice(startIndex, endIndex),
    }
  }, [filteredFiles, listScrollTop, listViewportHeight, shouldVirtualizeFlatList])

  const groupedVisibleFiles = useMemo<GroupedFileBucket[]>(() => {
    if (!isLargeChangeset) return []
    const buckets = new Map<string, FileChange[]>()
    for (const file of visibleFiles) {
      const folder = topLevelFolder(file.path)
      const list = buckets.get(folder)
      if (list) {
        list.push(file)
      } else {
        buckets.set(folder, [file])
      }
    }
    return Array.from(buckets.entries())
      .sort((a, b) => b[1].length - a[1].length || a[0].localeCompare(b[0]))
      .map(([folder, files]) => ({ folder, files }))
  }, [isLargeChangeset, visibleFiles])

  const toggleGroup = (folder: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev)
      if (next.has(folder)) {
        next.delete(folder)
      } else {
        next.add(folder)
      }
      return next
    })
  }

  const expandAllGroups = () => {
    setExpandedGroups(new Set(groupedVisibleFiles.map((g) => g.folder)))
  }

  const collapseAllGroups = () => {
    setExpandedGroups(new Set())
  }

  const handleStageGroup = async (folder: string, files: FileChange[]) => {
    if (!user) return
    const paths = files.filter((f) => !f.staged).map((f) => f.path)
    if (paths.length === 0) return
    setBusyGroupActionKey(`stage:${folder}`)
    try {
      await stageFiles(paths, user.login)
    } finally {
      setBusyGroupActionKey(null)
    }
  }

  const handleUnstageGroup = async (folder: string, files: FileChange[]) => {
    const paths = files.filter((f) => f.staged).map((f) => f.path)
    if (paths.length === 0) return
    setBusyGroupActionKey(`unstage:${folder}`)
    try {
      await unstageFiles(paths)
    } finally {
      setBusyGroupActionKey(null)
    }
  }

  return (
    <div className="h-full flex flex-col bg-surface-900/50 border-r border-surface-700/30">
      <div className="flex items-center justify-between px-4 py-3 border-b border-surface-700/30">
        <h3 className="text-[10px] font-medium text-surface-500 uppercase tracking-widest">
          Cambios ({effectiveFileChanges.length})
        </h3>
        <div className="flex gap-2">
          {someSelected ? (
            <>
              <button
                onClick={handleStageSelected}
                className="text-[11px] text-brand-400 hover:text-brand-300 flex items-center gap-1 transition-colors"
              >
                <Plus size={11} />
                Preparar ({selectedFiles.size})
              </button>
              <button
                onClick={deselectAll}
                className="text-[11px] text-surface-500 hover:text-surface-300 transition-colors"
              >
                Deseleccionar
              </button>
            </>
          ) : effectiveFileChanges.length > 0 ? (
            <>
              {hasUnstagedFiles && (
                <button
                  onClick={handlePrepareAll}
                  disabled={isPreparing}
                  className="text-[11px] text-brand-400 hover:text-brand-300 flex items-center gap-1 transition-colors disabled:opacity-50"
                >
                  {isPreparing ? (
                    <Loader2 size={11} className="animate-spin" />
                  ) : (
                    <Plus size={11} />
                  )}
                  Preparar todo ({unstagedCount})
                </button>
              )}
              {!isLargeChangeset ? (
                <button
                  onClick={handleSelectAllVisible}
                  className="text-[11px] text-surface-500 hover:text-surface-300 transition-colors"
                >
                  Seleccionar todo
                </button>
              ) : (
                <span className="text-[10px] text-surface-600">
                  Cambios masivos: usa selección por carpeta o búsqueda
                </span>
              )}
            </>
          ) : null}
        </div>
      </div>

      <div className="px-4 py-2 border-b border-surface-700/20 bg-surface-950/40">
        <label className="flex items-center gap-2 rounded-lg border border-surface-700/40 bg-surface-900/70 px-2.5 py-2">
          <Search size={13} className="text-surface-500 shrink-0" />
          <input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={isLargeChangeset ? 'Buscar archivo/ruta (recomendado con cambios masivos)' : 'Buscar archivo/ruta'}
            className="w-full bg-transparent text-[12px] text-surface-200 placeholder:text-surface-600 focus:outline-none"
          />
        </label>
        {normalizedQuery && (
          <p className="mt-1.5 text-[10px] text-surface-500">
            Coincidencias: {filteredFiles.length.toLocaleString()} / {effectiveFileChanges.length.toLocaleString()}
          </p>
        )}

        <div className="mt-2 flex flex-wrap gap-1.5">
          <button
            type="button"
            onClick={() => setNoiseFilterMode('all')}
            className={clsx(
              'text-[10px] px-2 py-1 rounded border transition-colors',
              noiseFilterMode === 'all'
                ? 'border-brand-500/40 bg-brand-500/10 text-brand-300'
                : 'border-white/6 bg-white/3 text-surface-300 hover:bg-white/5'
            )}
            title="Ver todos los cambios"
          >
            Todo ({gitgovVisibleFiles.length.toLocaleString()})
          </button>
          <button
            type="button"
            onClick={() => setNoiseFilterMode('code')}
            className={clsx(
              'text-[10px] px-2 py-1 rounded border transition-colors',
              noiseFilterMode === 'code'
                ? 'border-brand-500/40 bg-brand-500/10 text-brand-300'
                : 'border-white/6 bg-white/3 text-surface-300 hover:bg-white/5'
            )}
            title="Oculta archivos generados/cache detectados por heurística"
          >
            Solo código ({codeFilesCount.toLocaleString()})
          </button>
          <button
            type="button"
            onClick={() => setNoiseFilterMode('noise')}
            className={clsx(
              'text-[10px] px-2 py-1 rounded border transition-colors',
              noiseFilterMode === 'noise'
                ? 'border-warning-500/40 bg-warning-500/10 text-warning-300'
                : 'border-white/6 bg-white/3 text-surface-300 hover:bg-white/5'
            )}
            title="Ver solo archivos generados/cache detectados por heurística"
          >
            Solo ruido ({noiseFilesCount.toLocaleString()})
          </button>
        </div>

        {noiseFilterMode !== 'all' && (
          <p className="mt-1.5 text-[10px] text-surface-600">
            Filtro activo: {noiseFilterMode === 'code' ? 'solo código' : 'solo ruido generado'} (heurística, no altera Git)
          </p>
        )}

        {(gitgovIgnoreExists || gitgovIgnoreRules.length > 0 || gitgovHiddenCount > 0 || gitgovIgnoreLoadError) && (
          <div className="mt-2 rounded border border-white/6 bg-surface-950/45 p-2">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div>
                <p className="text-[10px] text-surface-300 font-medium">Ocultación GitGov (.gitgovignore)</p>
                <p className="text-[10px] text-surface-500">
                  reglas: {gitgovIgnoreRules.length} · ocultos por GitGov: {gitgovHiddenCount}
                </p>
              </div>
              <button
                type="button"
                onClick={() => setShowGitGovHidden((v) => !v)}
                className={clsx(
                  'text-[10px] px-2 py-1 rounded border transition-colors',
                  showGitGovHidden
                    ? 'border-warning-500/40 bg-warning-500/10 text-warning-300'
                    : 'border-brand-500/40 bg-brand-500/10 text-brand-300'
                )}
              >
                {showGitGovHidden ? 'Ocultar nuevamente (GitGov)' : 'Mostrar ocultos GitGov'}
              </button>
            </div>
            <p className="mt-1 text-[10px] text-surface-600">
              Semántica explícita: oculta solo en GitGov (no altera Git).
              {gitgovIgnorePath ? ` Archivo: ${gitgovIgnorePath}` : ''}
            </p>
            {gitgovHiddenTopRules.length > 0 && (
              <div className="mt-2">
                <p className="text-[10px] text-surface-500 mb-1">Reglas que están ocultando archivos (top)</p>
                <div className="flex flex-col gap-1.5">
                  {gitgovHiddenTopRules.map((item) => (
                    <div key={item.rule} className="flex flex-wrap items-center justify-between gap-2 rounded bg-white/4 px-2 py-1">
                      <div className="text-[10px] text-surface-300">
                        <code className="mono-data text-brand-300">{item.rule}</code>
                        <span className="ml-1 text-surface-500">({item.count})</span>
                      </div>
                      <div className="flex flex-wrap gap-1.5">
                        <button
                          type="button"
                          onClick={() => void moveGitGovRuleToTarget(item.rule, 'exclude')}
                          disabled={isApplyingIgnoreRules || isMovingGitGovRule !== null}
                          className="text-[9px] px-1.5 py-0.5 rounded bg-brand-500/12 text-brand-300 hover:bg-brand-500/22 transition-colors disabled:opacity-50"
                          title="Mover regla a .git/info/exclude"
                        >
                          {isMovingGitGovRule === item.rule ? '...' : 'Mover a .git/info/exclude'}
                        </button>
                        <button
                          type="button"
                          onClick={() => void moveGitGovRuleToTarget(item.rule, 'gitignore')}
                          disabled={isApplyingIgnoreRules || isMovingGitGovRule !== null}
                          className="text-[9px] px-1.5 py-0.5 rounded bg-white/6 text-surface-300 hover:bg-white/10 transition-colors disabled:opacity-50"
                          title="Mover regla a .gitignore"
                        >
                          {isMovingGitGovRule === item.rule ? '...' : 'Mover a .gitignore'}
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
            {gitgovIgnoreLoadError && <p className="mt-1 text-[10px] text-danger-400">{gitgovIgnoreLoadError}</p>}
          </div>
        )}
      </div>

      {isLargeChangeset && (
        <div className="px-4 py-2 border-b border-surface-700/30 bg-warning-500/5">
          <div className="flex items-start gap-2">
            <AlertCircle size={13} strokeWidth={1.75} className="text-warning-500 mt-0.5 shrink-0" />
            <div className="min-w-0">
              <p className="text-[11px] text-warning-300 font-medium">
                Cambios masivos detectados ({effectiveFileChanges.length.toLocaleString()} archivos)
              </p>
              <p className="text-[10px] text-surface-500 mt-0.5">
                Se renderiza una vista parcial para evitar bloqueos. Aun puedes preparar todo.
              </p>

              {analysis.topFolders.length > 0 && (
                <div className="mt-2">
                  <div className="flex items-center gap-1.5 text-[10px] text-surface-500 mb-1">
                    <FolderTree size={11} className="shrink-0" />
                    Carpetas con más cambios
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {analysis.topFolders.map((bucket) => (
                      <span key={bucket.folder} className="text-[10px] px-2 py-1 rounded bg-white/4 text-surface-300">
                        {bucket.folder}: {bucket.count.toLocaleString()}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {analysis.candidateFiles > 0 && (
                <div className="mt-3 rounded-lg border border-brand-500/20 bg-brand-500/5 p-2.5">
                  <div className="flex items-start gap-2">
                    <Sparkles size={12} className="text-brand-400 mt-0.5 shrink-0" />
                    <div className="min-w-0">
                      <p className="text-[11px] text-brand-300 font-medium">
                        Detectamos {analysis.candidateFiles.toLocaleString()} archivos generados/cache
                      </p>
                      <p className="text-[10px] text-surface-500 mt-0.5">
                        Heurística: {analysis.matchedSegments.slice(0, 6).join(', ')}
                        {analysis.matchedSegments.length > 6 ? ` +${analysis.matchedSegments.length - 6}` : ''}
                      </p>

                      {analysis.topNoiseFolders.length > 0 && (
                        <div className="flex flex-wrap gap-1.5 mt-2">
                          {analysis.topNoiseFolders.slice(0, 4).map((bucket) => (
                            <span key={bucket.folder} className="text-[10px] px-2 py-1 rounded bg-white/4 text-surface-300 mono-data">
                              {bucket.folder} ({bucket.count})
                            </span>
                          ))}
                        </div>
                      )}

                      <div className="flex flex-wrap gap-2 mt-2">
                        <button
                          type="button"
                          onClick={() => setShowIgnorePreview((v) => !v)}
                          className="text-[10px] px-2 py-1 rounded bg-white/5 text-surface-300 hover:bg-white/8 transition-colors"
                        >
                          {showIgnorePreview ? 'Ocultar reglas sugeridas' : 'Generar reglas recomendadas'}
                        </button>
                        <button
                          type="button"
                          onClick={() => applyIgnoreRules('exclude')}
                          disabled={isApplyingIgnoreRules}
                          className="text-[10px] px-2 py-1 rounded bg-brand-500/15 text-brand-300 hover:bg-brand-500/25 transition-colors disabled:opacity-50"
                          title="Local-only. No se commitea."
                        >
                          Aplicar a .git/info/exclude (local)
                        </button>
                        <button
                          type="button"
                          onClick={() => applyIgnoreRules('gitignore')}
                          disabled={isApplyingIgnoreRules}
                          className="text-[10px] px-2 py-1 rounded bg-white/5 text-surface-300 hover:bg-white/8 transition-colors disabled:opacity-50"
                          title="Cambiará el repo (archivo versionable)."
                        >
                          Aplicar a .gitignore
                        </button>
                        <button
                          type="button"
                          onClick={() => applyRulesWithConfirmation('gitgovignore', analysis.rules.map((r) => r.rule), 'heurística de limpieza')}
                          disabled={isApplyingIgnoreRules}
                          className="text-[10px] px-2 py-1 rounded bg-warning-500/10 text-warning-300 hover:bg-warning-500/20 transition-colors disabled:opacity-50"
                          title="Oculta solo en GitGov (.gitgovignore)"
                        >
                          Ocultar solo en GitGov (.gitgovignore)
                        </button>
                      </div>

                      {showIgnorePreview && (
                        <div className="mt-2 rounded border border-white/6 bg-surface-950/60 p-2">
                          <p className="text-[10px] text-surface-500 mb-1">
                            Reglas sugeridas ({analysis.rules.length}) — preview (no se aplican hasta que hagas click en un botón)
                          </p>
                          <div className="max-h-28 overflow-y-auto space-y-1">
                            {analysis.rules.slice(0, 20).map((rule) => (
                              <div key={rule.rule} className="flex items-center justify-between gap-2 text-[10px]">
                                <code className="text-brand-300 mono-data">{rule.rule}</code>
                                <span className="text-surface-500 shrink-0">{rule.count}</span>
                              </div>
                            ))}
                            {analysis.rules.length > 20 && (
                              <p className="text-[10px] text-surface-600">+{analysis.rules.length - 20} reglas más</p>
                            )}
                          </div>
                        </div>
                      )}

                      {ignoreRulesError && (
                        <p className="mt-2 text-[10px] text-danger-400">{ignoreRulesError}</p>
                      )}
                      {ignoreRulesSuccess && (
                        <p className="mt-2 text-[10px] text-success-400">{ignoreRulesSuccess}</p>
                      )}
                      {lastHiddenByRules !== null && (
                        <p className="mt-1 text-[10px] text-surface-500">
                          Transparencia: ocultados por reglas (última aplicación) = {lastHiddenByRules}
                        </p>
                      )}
                    </div>
                  </div>
                </div>
              )}

              {analysis.stackTemplates.length > 0 && (
                <div className="mt-3 rounded-lg border border-white/6 bg-surface-950/45 p-2.5">
                  <div className="flex items-start gap-2">
                    <Sparkles size={12} className="text-brand-400 mt-0.5 shrink-0" />
                    <div className="min-w-0 w-full">
                      <p className="text-[11px] text-surface-200 font-medium">
                        Packs sugeridos por stack (preview)
                      </p>
                      <p className="text-[10px] text-surface-500 mt-0.5">
                        Reglas estándar para limpiar ruido de build/cache según la tecnología detectada.
                      </p>

                      <div className="flex flex-wrap gap-1.5 mt-2">
                        {analysis.stackTemplates.map((tpl) => (
                          <button
                            key={tpl.id}
                            type="button"
                            onClick={() => {
                              setSelectedStackTemplateId(tpl.id)
                              setSelectedStackRuleGroupId(null)
                            }}
                            className={clsx(
                              'text-[10px] px-2 py-1 rounded border transition-colors',
                              selectedStackTemplateId === tpl.id
                                ? 'border-brand-500/40 bg-brand-500/10 text-brand-300'
                                : 'border-white/6 bg-white/3 text-surface-300 hover:bg-white/5'
                            )}
                          >
                            {tpl.label} ({tpl.rules.length})
                          </button>
                        ))}
                      </div>

                      {selectedStackTemplate && (
                        <div className="mt-2 rounded border border-white/6 bg-surface-950/60 p-2">
                          <div className="flex flex-wrap items-center justify-between gap-2">
                            <div>
                              <p className="text-[10px] text-surface-300 font-medium">
                                Pack {selectedStackTemplate.label}
                              </p>
                              <p className="text-[10px] text-surface-500">
                                Detectado por: {selectedStackTemplate.reasons.join(', ')}
                              </p>
                            </div>
                            <div className="flex flex-wrap gap-2">
                              <button
                                type="button"
                                onClick={exportGitGovIgnorePreset}
                                className="text-[10px] px-2 py-1 rounded bg-white/5 text-surface-300 hover:bg-white/8 transition-colors"
                                title="Exportar preset como archivo de texto para compartir con el equipo"
                              >
                                Exportar preset .gitgovignore
                              </button>
                              <button
                                type="button"
                                onClick={() => applyStackTemplateRules('exclude')}
                                disabled={isApplyingIgnoreRules}
                                className="text-[10px] px-2 py-1 rounded bg-brand-500/15 text-brand-300 hover:bg-brand-500/25 transition-colors disabled:opacity-50"
                                title="Local-only. No se commitea."
                              >
                                Aplicar pack a .git/info/exclude
                              </button>
                              <button
                                type="button"
                                onClick={() => applyStackTemplateRules('gitignore')}
                                disabled={isApplyingIgnoreRules}
                                className="text-[10px] px-2 py-1 rounded bg-white/5 text-surface-300 hover:bg-white/8 transition-colors disabled:opacity-50"
                                title="Cambiará el repo (archivo versionable)."
                              >
                                Aplicar pack a .gitignore
                              </button>
                              <button
                                type="button"
                                onClick={() => applyStackTemplateRules('gitgovignore')}
                                disabled={isApplyingIgnoreRules}
                                className="text-[10px] px-2 py-1 rounded bg-warning-500/10 text-warning-300 hover:bg-warning-500/20 transition-colors disabled:opacity-50"
                                title="Oculta solo en GitGov (.gitgovignore)"
                              >
                                Aplicar pack a .gitgovignore
                              </button>
                            </div>
                          </div>

                          {selectedStackTemplate.ruleGroups.length > 1 && (
                            <div className="mt-2 flex flex-wrap gap-1.5">
                              {selectedStackTemplate.ruleGroups.map((group) => (
                                <button
                                  key={`${selectedStackTemplate.id}:${group.id}`}
                                  type="button"
                                  onClick={() => setSelectedStackRuleGroupId(group.id)}
                                  className={clsx(
                                    'text-[10px] px-2 py-1 rounded border transition-colors',
                                    selectedStackRuleGroup?.id === group.id
                                      ? 'border-brand-500/40 bg-brand-500/10 text-brand-300'
                                      : 'border-white/6 bg-white/3 text-surface-300 hover:bg-white/5'
                                  )}
                                  title={group.description}
                                >
                                  {group.label} ({group.rules.length})
                                </button>
                              ))}
                            </div>
                          )}

                          {selectedStackRuleGroup && (
                            <p className="mt-2 text-[10px] text-surface-500">
                              Sub-pack activo: <span className="text-surface-300">{selectedStackRuleGroup.label}</span>
                              {selectedStackRuleGroup.description ? ` · ${selectedStackRuleGroup.description}` : ''}
                            </p>
                          )}

                          <div className="mt-2 max-h-24 overflow-y-auto space-y-1">
                            {(selectedStackRuleGroup?.rules ?? selectedStackTemplate.rules).map((rule) => (
                              <div key={rule} className="text-[10px] flex items-center justify-between gap-2">
                                <code className="text-brand-300 mono-data">{rule}</code>
                              </div>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              )}

              <div className="flex flex-wrap items-center gap-2 mt-2">
                <span className="text-[10px] text-surface-600">
                  Modo robusto: acciones por carpeta + render parcial
                </span>
                {groupedVisibleFiles.length > 1 && (
                  <>
                    <button
                      type="button"
                      onClick={expandAllGroups}
                      className="text-[10px] px-2 py-1 rounded bg-white/5 text-surface-300 hover:bg-white/8 transition-colors"
                    >
                      Expandir carpetas ({groupedVisibleFiles.length})
                    </button>
                    <button
                      type="button"
                      onClick={collapseAllGroups}
                      className="text-[10px] px-2 py-1 rounded bg-white/5 text-surface-400 hover:text-surface-200 hover:bg-white/8 transition-colors"
                    >
                      Colapsar carpetas
                    </button>
                  </>
                )}
                {hiddenFilesCount > 0 && (
                  <button
                    type="button"
                    onClick={() => setVisibleCount((prev) => Math.min(prev + VISIBLE_ROWS_STEP, filteredFiles.length))}
                    className="text-[10px] px-2 py-1 rounded bg-white/5 text-surface-300 hover:bg-white/8 transition-colors"
                  >
                    Mostrar +{Math.min(VISIBLE_ROWS_STEP, hiddenFilesCount)}
                  </button>
                )}
                {hiddenFilesCount > 0 && (
                  <button
                    type="button"
                    onClick={() => setVisibleCount(filteredFiles.length)}
                    className="text-[10px] px-2 py-1 rounded bg-white/5 text-surface-400 hover:text-surface-200 hover:bg-white/8 transition-colors"
                  >
                    Mostrar todo (puede ser lento)
                  </button>
                )}
                {visibleCount > INITIAL_VISIBLE_ROWS && (
                  <button
                    type="button"
                    onClick={() => setVisibleCount(INITIAL_VISIBLE_ROWS)}
                    className="text-[10px] px-2 py-1 rounded bg-white/5 text-surface-500 hover:text-surface-300 hover:bg-white/8 transition-colors"
                  >
                    Colapsar vista
                  </button>
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      <div
        ref={listContainerRef}
        onScroll={(e) => setListScrollTop(e.currentTarget.scrollTop)}
        className="flex-1 overflow-y-auto divide-y divide-surface-700/15"
      >
        {isLoadingStatus && effectiveFileChanges.length === 0 ? (
          <div className="space-y-0.5">
            {[1, 2, 3, 4, 5].map((i) => (
              <SkeletonFileRow key={i} />
            ))}
          </div>
        ) : effectiveFileChanges.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-surface-500 p-6">
            <FileCode size={24} strokeWidth={1.5} className="mb-3 text-surface-700" />
            <p className="text-xs font-medium text-surface-400">No hay cambios</p>
            <p className="text-[11px] text-surface-600 mt-1">Edita archivos para empezar</p>
          </div>
        ) : filteredFiles.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-surface-500 p-6">
            <Search size={22} strokeWidth={1.5} className="mb-3 text-surface-700" />
            <p className="text-xs font-medium text-surface-400">Sin coincidencias</p>
            <p className="text-[11px] text-surface-600 mt-1">Prueba otro término de búsqueda</p>
          </div>
        ) : (
          isLargeChangeset ? (
            groupedVisibleFiles.map((group) => {
              const isExpanded = normalizedQuery ? true : expandedGroups.has(group.folder)
              const stagedInGroup = group.files.reduce((acc, file) => acc + (file.staged ? 1 : 0), 0)
              const unstagedInGroup = group.files.length - stagedInGroup
              const stageBusy = busyGroupActionKey === `stage:${group.folder}`
              const unstageBusy = busyGroupActionKey === `unstage:${group.folder}`
              return (
                <div key={group.folder} className="border-b border-surface-700/15 last:border-b-0">
                  <div className="w-full flex items-center justify-between gap-2 px-3 py-2 bg-surface-950/35 hover:bg-surface-950/55 transition-colors">
                    <button
                      type="button"
                      onClick={() => toggleGroup(group.folder)}
                      className="flex items-center gap-2 min-w-0 flex-1 text-left"
                    >
                      {isExpanded ? (
                        <ChevronDown size={13} className="text-surface-500 shrink-0" />
                      ) : (
                        <ChevronRight size={13} className="text-surface-500 shrink-0" />
                      )}
                      <span className="text-[11px] text-surface-300 font-medium truncate">{group.folder}</span>
                    </button>
                    <div className="flex items-center gap-1.5 shrink-0">
                      {user && unstagedInGroup > 0 && (
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation()
                            void handleStageGroup(group.folder, group.files)
                          }}
                          disabled={stageBusy || !!busyGroupActionKey}
                          className="text-[9px] px-1.5 py-0.5 rounded bg-brand-500/12 text-brand-300 hover:bg-brand-500/22 transition-colors disabled:opacity-50"
                          title={`Preparar ${unstagedInGroup} archivo(s) en ${group.folder}`}
                        >
                          {stageBusy ? '...' : `Preparar ${unstagedInGroup}`}
                        </button>
                      )}
                      {stagedInGroup > 0 && (
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation()
                            void handleUnstageGroup(group.folder, group.files)
                          }}
                          disabled={unstageBusy || !!busyGroupActionKey}
                          className="text-[9px] px-1.5 py-0.5 rounded bg-white/6 text-surface-300 hover:bg-white/10 transition-colors disabled:opacity-50"
                          title={`Quitar del staging ${stagedInGroup} archivo(s) en ${group.folder}`}
                        >
                          {unstageBusy ? '...' : `Quitar ${stagedInGroup}`}
                        </button>
                      )}
                      {stagedInGroup > 0 && (
                        <span className="text-[9px] px-1.5 py-0.5 rounded bg-brand-500/10 text-brand-300">
                          staged {stagedInGroup}
                        </span>
                      )}
                      <span className="text-[10px] text-surface-500 mono-data">{group.files.length}</span>
                    </div>
                  </div>

                  {isExpanded && (
                    <div className="divide-y divide-surface-700/10">
                      {group.files.map((file) => (
                        <FileItem
                          key={file.path}
                          file={file}
                          selected={selectedFiles.has(file.path)}
                          disabled={false}
                          showGitGovHiddenBadge={showGitGovHidden}
                          gitgovHiddenRule={gitgovHiddenRuleByPath.get(file.path) ?? null}
                          onToggle={() => handleToggle(file.path, selectedFiles.has(file.path))}
                          onViewDiff={() => loadDiff(file.path)}
                          onUnstage={() => unstageFiles([file.path])}
                        />
                      ))}
                    </div>
                  )}
                </div>
              )
            })
          ) : shouldVirtualizeFlatList ? (
            <div className="divide-y divide-surface-700/15">
              {flatVirtualWindow.topPadding > 0 && (
                <div
                  aria-hidden="true"
                  style={{ height: flatVirtualWindow.topPadding }}
                  className="pointer-events-none"
                />
              )}

              {flatVirtualWindow.files.map((file) => (
                <FileItem
                  key={file.path}
                  file={file}
                  selected={selectedFiles.has(file.path)}
                  disabled={false}
                  showGitGovHiddenBadge={showGitGovHidden}
                  gitgovHiddenRule={gitgovHiddenRuleByPath.get(file.path) ?? null}
                  onToggle={() => handleToggle(file.path, selectedFiles.has(file.path))}
                  onViewDiff={() => loadDiff(file.path)}
                  onUnstage={() => unstageFiles([file.path])}
                />
              ))}

              {flatVirtualWindow.bottomPadding > 0 && (
                <div
                  aria-hidden="true"
                  style={{ height: flatVirtualWindow.bottomPadding }}
                  className="pointer-events-none"
                />
              )}
            </div>
          ) : (
            visibleFiles.map((file) => (
              <FileItem
                key={file.path}
                file={file}
                selected={selectedFiles.has(file.path)}
                disabled={false}
                showGitGovHiddenBadge={showGitGovHidden}
                gitgovHiddenRule={gitgovHiddenRuleByPath.get(file.path) ?? null}
                onToggle={() => handleToggle(file.path, selectedFiles.has(file.path))}
                onViewDiff={() => loadDiff(file.path)}
                onUnstage={() => unstageFiles([file.path])}
              />
            ))
          )
        )}
      </div>

      {stagedFiles.size > 0 && (
        <div className="px-4 py-2.5 border-t border-surface-700/30">
          <p className="text-[11px] text-brand-400 font-medium mono-data">
            {stagedFiles.size} archivo{stagedFiles.size !== 1 ? 's' : ''} en staging
          </p>
          {hiddenFilesCount > 0 && (
            <p className="text-[10px] text-surface-500 mt-1">
              Mostrando {visibleFiles.length.toLocaleString()} de {filteredFiles.length.toLocaleString()}
              {normalizedQuery ? ` coincidencias (de ${effectiveFileChanges.length.toLocaleString()} cambios)` : ` cambios`}
            </p>
          )}
          {!isLargeChangeset && shouldVirtualizeFlatList && (
            <p className="text-[10px] text-surface-600 mt-1">
              Render virtualizado activo para {filteredFiles.length.toLocaleString()} cambios (mejor rendimiento)
            </p>
          )}
        </div>
      )}
    </div>
  )
}
