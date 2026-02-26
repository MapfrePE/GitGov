import type { FileChange } from '@/lib/types'

export interface IgnoreRuleSuggestion {
  rule: string
  count: number
  kind: 'generated_dir' | 'cache_dir' | 'python_cache'
  sourceSegment: string
}

export interface FolderBucket {
  folder: string
  count: number
}

export type StackTemplateId = 'nextjs' | 'node' | 'rust' | 'python' | 'java'

export interface StackTemplateRuleGroup {
  id: string
  label: string
  rules: string[]
  description?: string
}

export interface StackTemplateSuggestion {
  id: StackTemplateId
  label: string
  rules: string[]
  reasons: string[]
  ruleGroups: StackTemplateRuleGroup[]
}

export interface LargeChangesetAnalysis {
  totalFiles: number
  candidateFiles: number
  rules: IgnoreRuleSuggestion[]
  topFolders: FolderBucket[]
  topNoiseFolders: FolderBucket[]
  matchedSegments: string[]
  stackTemplates: StackTemplateSuggestion[]
}

const GENERATED_SEGMENTS: Record<string, IgnoreRuleSuggestion['kind']> = {
  node_modules: 'generated_dir',
  '.next': 'generated_dir',
  dist: 'generated_dir',
  build: 'generated_dir',
  out: 'generated_dir',
  target: 'generated_dir',
  coverage: 'generated_dir',
  '.turbo': 'cache_dir',
  '.cache': 'cache_dir',
  '.parcel-cache': 'cache_dir',
  '.vite': 'cache_dir',
  '.gradle': 'cache_dir',
  '.pytest_cache': 'python_cache',
  '__pycache__': 'python_cache',
  '.mypy_cache': 'python_cache',
  '.ruff_cache': 'python_cache',
  '.venv': 'python_cache',
  venv: 'python_cache',
}

const STACK_TEMPLATES: Record<StackTemplateId, { label: string; groups: StackTemplateRuleGroup[] }> = {
  nextjs: {
    label: 'Next.js',
    groups: [
      {
        id: 'cache',
        label: 'Cache / build interno',
        rules: ['.next/', '.vercel/', '*.tsbuildinfo'],
      },
      {
        id: 'exports',
        label: 'Build export',
        rules: ['out/'],
      },
      {
        id: 'full',
        label: 'Pack completo',
        rules: ['.next/', '.vercel/', '*.tsbuildinfo', 'out/'],
      },
    ],
  },
  node: {
    label: 'Node.js',
    groups: [
      {
        id: 'deps',
        label: 'Dependencias',
        rules: ['node_modules/'],
      },
      {
        id: 'build',
        label: 'Build outputs',
        rules: ['dist/', 'build/'],
      },
      {
        id: 'tests',
        label: 'Cobertura / logs',
        rules: ['coverage/', 'npm-debug.log*', 'yarn-error.log*', 'pnpm-debug.log*'],
      },
      {
        id: 'full',
        label: 'Pack completo',
        rules: ['node_modules/', 'dist/', 'build/', 'coverage/', 'npm-debug.log*', 'yarn-error.log*', 'pnpm-debug.log*'],
      },
    ],
  },
  rust: {
    label: 'Rust',
    groups: [
      {
        id: 'build',
        label: 'Build outputs',
        rules: ['target/'],
      },
      {
        id: 'full',
        label: 'Pack completo',
        rules: ['target/'],
      },
    ],
  },
  python: {
    label: 'Python',
    groups: [
      {
        id: 'bytecode',
        label: 'Bytecode / caches',
        rules: ['__pycache__/', '*.py[cod]', '.pytest_cache/', '.mypy_cache/', '.ruff_cache/'],
      },
      {
        id: 'venv',
        label: 'Virtual envs',
        rules: ['.venv/', 'venv/'],
      },
      {
        id: 'build',
        label: 'Build outputs',
        rules: ['build/', 'dist/'],
      },
      {
        id: 'full',
        label: 'Pack completo',
        rules: ['__pycache__/', '*.py[cod]', '.venv/', 'venv/', '.pytest_cache/', '.mypy_cache/', '.ruff_cache/', 'build/', 'dist/'],
      },
    ],
  },
  java: {
    label: 'Java / JVM',
    groups: [
      {
        id: 'gradle',
        label: 'Gradle cache',
        rules: ['.gradle/'],
      },
      {
        id: 'build',
        label: 'Build outputs',
        rules: ['build/', 'out/', 'target/', '*.class'],
      },
      {
        id: 'full',
        label: 'Pack completo',
        rules: ['.gradle/', 'build/', 'out/', 'target/', '*.class'],
      },
    ],
  },
}

function splitPath(p: string): string[] {
  return p.split(/[\\/]+/).filter(Boolean)
}

export function topLevelFolder(path: string): string {
  const segments = splitPath(path)
  if (segments.length <= 1) return '(root)'
  return segments[0]
}

function ruleFromPath(path: string): { rule: string; segment: string; kind: IgnoreRuleSuggestion['kind'] } | null {
  const segments = splitPath(path)
  for (let i = 0; i < segments.length; i += 1) {
    const segment = segments[i]
    const kind = GENERATED_SEGMENTS[segment]
    if (!kind) continue
    return {
      rule: `${segments.slice(0, i + 1).join('/')}/`,
      segment,
      kind,
    }
  }
  return null
}

export function detectGeneratedNoiseRule(path: string): { rule: string; segment: string; kind: IgnoreRuleSuggestion['kind'] } | null {
  return ruleFromPath(path)
}

export function isLikelyGeneratedNoisePath(path: string): boolean {
  return ruleFromPath(path) !== null
}

export function parseGitGovIgnoreRules(content: string): string[] {
  return content
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith('#'))
}

function escapeRegexChar(ch: string): string {
  return /[\\^$+?.()|{}]/.test(ch) ? `\\${ch}` : ch
}

function globToRegexSource(glob: string): string {
  let out = ''
  for (let i = 0; i < glob.length; i += 1) {
    const ch = glob[i]
    if (ch === '*') {
      if (glob[i + 1] === '*') {
        out += '.*'
        i += 1
      } else {
        out += '[^/]*'
      }
      continue
    }
    if (ch === '?') {
      out += '.'
      continue
    }
    if (ch === '[') {
      const end = glob.indexOf(']', i + 1)
      if (end > i) {
        out += glob.slice(i, end + 1)
        i = end
        continue
      }
    }
    out += escapeRegexChar(ch)
  }
  return out
}

export function matchesGitGovIgnoreRule(path: string, rule: string): boolean {
  const normalizedPath = path.replace(/\\/g, '/')
  const trimmedRule = rule.trim()
  if (!trimmedRule) return false

  if (trimmedRule.endsWith('/')) {
    const dirRule = trimmedRule.slice(0, -1)
    if (!dirRule) return false
    const normalizedDirRule = dirRule.replace(/\\/g, '/')

    if (!normalizedDirRule.includes('/')) {
      const segments = normalizedPath.split('/').filter(Boolean)
      return segments.includes(normalizedDirRule)
    }

    return normalizedPath === normalizedDirRule || normalizedPath.startsWith(`${normalizedDirRule}/`)
  }

  const hasSlash = trimmedRule.includes('/')
  const regex = new RegExp(`^${globToRegexSource(trimmedRule.replace(/\\/g, '/'))}$`)
  if (hasSlash) {
    return regex.test(normalizedPath)
  }

  const baseName = normalizedPath.split('/').pop() ?? normalizedPath
  return regex.test(baseName)
}

export function firstMatchingGitGovIgnoreRule(path: string, rules: string[]): string | null {
  for (const rule of rules) {
    if (matchesGitGovIgnoreRule(path, rule)) return rule
  }
  return null
}

export function isHiddenByGitGovIgnore(path: string, rules: string[]): boolean {
  return firstMatchingGitGovIgnoreRule(path, rules) !== null
}

function toSortedBuckets(map: Map<string, number>, limit = 5): FolderBucket[] {
  return Array.from(map.entries())
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, limit)
    .map(([folder, count]) => ({ folder, count }))
}

function uniqueRules(rules: string[]): string[] {
  return Array.from(new Set(rules))
}

function detectStackTemplates(fileChanges: FileChange[]): StackTemplateSuggestion[] {
  const markers = {
    hasPackageJson: false,
    hasNextConfig: false,
    hasNodeModules: false,
    hasCargoToml: false,
    hasRustSource: false,
    hasPyProject: false,
    hasRequirements: false,
    hasPythonFiles: false,
    hasPomXml: false,
    hasGradle: false,
    hasJavaSource: false,
  }

  const pathSet = new Set<string>()
  for (const file of fileChanges) {
    const path = file.path.replace(/\\/g, '/')
    pathSet.add(path)
    const lower = path.toLowerCase()

    if (lower === 'package.json' || lower.endsWith('/package.json')) markers.hasPackageJson = true
    if (lower.includes('/node_modules/') || lower.startsWith('node_modules/')) markers.hasNodeModules = true
    if (lower === 'next.config.js' || lower === 'next.config.mjs' || lower === 'next.config.ts') markers.hasNextConfig = true

    if (lower === 'cargo.toml' || lower.endsWith('/cargo.toml')) markers.hasCargoToml = true
    if (lower.endsWith('.rs')) markers.hasRustSource = true

    if (lower === 'pyproject.toml' || lower.endsWith('/pyproject.toml')) markers.hasPyProject = true
    if (lower === 'requirements.txt' || lower.endsWith('/requirements.txt')) markers.hasRequirements = true
    if (lower.endsWith('.py') || lower.includes('/__pycache__/') || lower.startsWith('__pycache__/')) markers.hasPythonFiles = true

    if (lower === 'pom.xml' || lower.endsWith('/pom.xml')) markers.hasPomXml = true
    if (lower === 'build.gradle' || lower === 'build.gradle.kts' || lower.endsWith('/build.gradle') || lower.endsWith('/build.gradle.kts')) markers.hasGradle = true
    if (lower.endsWith('.java') || lower.includes('/src/main/java/') || lower.includes('/src/test/java/')) markers.hasJavaSource = true
  }

  const suggestions: StackTemplateSuggestion[] = []

  const pushTemplate = (id: StackTemplateId, reasons: string[]) => {
    const tpl = STACK_TEMPLATES[id]
    suggestions.push({
      id,
      label: tpl.label,
      rules: uniqueRules(tpl.groups.flatMap((g) => g.rules)),
      reasons,
      ruleGroups: tpl.groups.map((g) => ({
        id: g.id,
        label: g.label,
        rules: [...g.rules],
        description: g.description,
      })),
    })
  }

  if (markers.hasNextConfig || pathSet.has('.next') || markers.hasNodeModules || Array.from(pathSet).some((p) => p.startsWith('.next/'))) {
    const reasons = []
    if (markers.hasNextConfig) reasons.push('next.config.* detectado')
    if (Array.from(pathSet).some((p) => p.startsWith('.next/'))) reasons.push('.next/ detectado')
    if (markers.hasPackageJson) reasons.push('package.json presente')
    pushTemplate('nextjs', reasons.length ? reasons : ['Patrón compatible con Next.js'])
  }

  if (markers.hasPackageJson || markers.hasNodeModules) {
    const reasons = []
    if (markers.hasPackageJson) reasons.push('package.json detectado')
    if (markers.hasNodeModules) reasons.push('node_modules detectado')
    pushTemplate('node', reasons.length ? reasons : ['Patrón compatible con Node.js'])
  }

  if (markers.hasCargoToml || markers.hasRustSource) {
    const reasons = []
    if (markers.hasCargoToml) reasons.push('Cargo.toml detectado')
    if (markers.hasRustSource) reasons.push('archivos .rs detectados')
    pushTemplate('rust', reasons.length ? reasons : ['Patrón compatible con Rust'])
  }

  if (markers.hasPyProject || markers.hasRequirements || markers.hasPythonFiles) {
    const reasons = []
    if (markers.hasPyProject) reasons.push('pyproject.toml detectado')
    if (markers.hasRequirements) reasons.push('requirements.txt detectado')
    if (markers.hasPythonFiles) reasons.push('archivos Python/cache detectados')
    pushTemplate('python', reasons.length ? reasons : ['Patrón compatible con Python'])
  }

  if (markers.hasPomXml || markers.hasGradle || markers.hasJavaSource) {
    const reasons = []
    if (markers.hasPomXml) reasons.push('pom.xml detectado')
    if (markers.hasGradle) reasons.push('Gradle detectado')
    if (markers.hasJavaSource) reasons.push('archivos Java detectados')
    pushTemplate('java', reasons.length ? reasons : ['Patrón compatible con Java/JVM'])
  }

  return suggestions
}

export function analyzeLargeChangeset(fileChanges: FileChange[]): LargeChangesetAnalysis {
  const ruleCounts = new Map<string, IgnoreRuleSuggestion>()
  const folderCounts = new Map<string, number>()
  const noiseFolderCounts = new Map<string, number>()
  const matchedSegments = new Set<string>()

  let candidateFiles = 0

  for (const file of fileChanges) {
    folderCounts.set(topLevelFolder(file.path), (folderCounts.get(topLevelFolder(file.path)) ?? 0) + 1)

    const match = ruleFromPath(file.path)
    if (!match) continue

    candidateFiles += 1
    matchedSegments.add(match.segment)
    noiseFolderCounts.set(match.rule, (noiseFolderCounts.get(match.rule) ?? 0) + 1)

    const existing = ruleCounts.get(match.rule)
    if (existing) {
      existing.count += 1
    } else {
      ruleCounts.set(match.rule, {
        rule: match.rule,
        count: 1,
        kind: match.kind,
        sourceSegment: match.segment,
      })
    }
  }

  const rules = Array.from(ruleCounts.values()).sort((a, b) => b.count - a.count || a.rule.localeCompare(b.rule))

  return {
    totalFiles: fileChanges.length,
    candidateFiles,
    rules,
    topFolders: toSortedBuckets(folderCounts),
    topNoiseFolders: toSortedBuckets(noiseFolderCounts),
    matchedSegments: Array.from(matchedSegments).sort(),
    stackTemplates: detectStackTemplates(fileChanges),
  }
}
