export const COMMIT_TYPES = [
  { value: 'feat', label: 'Feature', description: 'Nueva funcionalidad' },
  { value: 'fix', label: 'Fix', description: 'Corrección de bug' },
  { value: 'docs', label: 'Docs', description: 'Documentación' },
  { value: 'style', label: 'Style', description: 'Formato, punto y coma, etc' },
  { value: 'refactor', label: 'Refactor', description: 'Refactorización de código' },
  { value: 'test', label: 'Test', description: 'Añadir o modificar tests' },
  { value: 'chore', label: 'Chore', description: 'Tareas de mantenimiento' },
  { value: 'hotfix', label: 'Hotfix', description: 'Corrección urgente en producción' },
] as const

export const STATUS_COLORS: Record<string, string> = {
  Success: 'bg-success-500',
  Blocked: 'bg-danger-500',
  Failed: 'bg-warning-500',
}

export const ACTION_COLORS: Record<string, string> = {
  Push: 'bg-brand-500',
  BranchCreate: 'bg-success-500',
  StageFile: 'bg-surface-500',
  Commit: 'bg-warning-500',
  BlockedPush: 'bg-danger-500',
  BlockedBranch: 'bg-danger-500',
}

export const FILE_STATUS_COLORS: Record<string, string> = {
  M: 'text-warning-500',
  A: 'text-success-500',
  D: 'text-danger-500',
  R: 'text-brand-500',
  '?': 'text-surface-400',
}

export const FILE_STATUS_LABELS: Record<string, string> = {
  M: 'Modified',
  A: 'Added',
  D: 'Deleted',
  R: 'Renamed',
  '?': 'Untracked',
}
