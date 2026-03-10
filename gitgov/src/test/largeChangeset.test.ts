import { describe, it, expect } from 'vitest'
import {
  topLevelFolder,
  detectGeneratedNoiseRule,
  isLikelyGeneratedNoisePath,
  parseGitGovIgnoreRules,
  matchesGitGovIgnoreRule,
  firstMatchingGitGovIgnoreRule,
  isHiddenByGitGovIgnore,
  analyzeLargeChangeset,
} from '@/lib/largeChangeset'
import type { FileChange } from '@/lib/types'

function fc(path: string): FileChange {
  return { path, status: 'Modified', staged: false }
}

describe('topLevelFolder', () => {
  it('returns (root) for root-level files', () => {
    expect(topLevelFolder('README.md')).toBe('(root)')
  })

  it('returns top-level directory name', () => {
    expect(topLevelFolder('src/main.ts')).toBe('src')
    expect(topLevelFolder('node_modules/react/index.js')).toBe('node_modules')
  })

  it('handles nested paths', () => {
    expect(topLevelFolder('a/b/c/d.ts')).toBe('a')
  })

  it('handles backslash paths', () => {
    expect(topLevelFolder('src\\components\\App.tsx')).toBe('src')
  })
})

describe('detectGeneratedNoiseRule', () => {
  it('detects node_modules', () => {
    const result = detectGeneratedNoiseRule('node_modules/react/index.js')
    expect(result).not.toBeNull()
    expect(result!.segment).toBe('node_modules')
    expect(result!.kind).toBe('generated_dir')
    expect(result!.rule).toBe('node_modules/')
  })

  it('detects nested .next directory', () => {
    const result = detectGeneratedNoiseRule('app/.next/cache/webpack/hot.js')
    expect(result).not.toBeNull()
    expect(result!.segment).toBe('.next')
    expect(result!.rule).toBe('app/.next/')
  })

  it('detects __pycache__', () => {
    const result = detectGeneratedNoiseRule('src/__pycache__/module.pyc')
    expect(result).not.toBeNull()
    expect(result!.kind).toBe('python_cache')
  })

  it('detects .gradle cache', () => {
    const result = detectGeneratedNoiseRule('.gradle/caches/something')
    expect(result).not.toBeNull()
    expect(result!.kind).toBe('cache_dir')
  })

  it('returns null for normal source files', () => {
    expect(detectGeneratedNoiseRule('src/main.ts')).toBeNull()
    expect(detectGeneratedNoiseRule('README.md')).toBeNull()
    expect(detectGeneratedNoiseRule('lib/utils.py')).toBeNull()
  })

  it('detects dist directory', () => {
    const result = detectGeneratedNoiseRule('dist/bundle.js')
    expect(result).not.toBeNull()
    expect(result!.segment).toBe('dist')
  })

  it('detects target (Rust build)', () => {
    const result = detectGeneratedNoiseRule('target/debug/binary')
    expect(result).not.toBeNull()
    expect(result!.segment).toBe('target')
  })
})

describe('isLikelyGeneratedNoisePath', () => {
  it('returns true for generated paths', () => {
    expect(isLikelyGeneratedNoisePath('node_modules/x')).toBe(true)
    expect(isLikelyGeneratedNoisePath('dist/app.js')).toBe(true)
    expect(isLikelyGeneratedNoisePath('.cache/data')).toBe(true)
  })

  it('returns false for source paths', () => {
    expect(isLikelyGeneratedNoisePath('src/app.ts')).toBe(false)
    expect(isLikelyGeneratedNoisePath('package.json')).toBe(false)
  })
})

describe('parseGitGovIgnoreRules', () => {
  it('parses rules from multiline content', () => {
    const content = 'node_modules/\n# comment\ndist/\n\nbuild/'
    const rules = parseGitGovIgnoreRules(content)
    expect(rules).toEqual(['node_modules/', 'dist/', 'build/'])
  })

  it('ignores comments and empty lines', () => {
    const content = '# ignore this\n\n  \n# also this\nkeep/'
    expect(parseGitGovIgnoreRules(content)).toEqual(['keep/'])
  })

  it('handles Windows line endings', () => {
    const content = 'a/\r\nb/\r\nc/'
    expect(parseGitGovIgnoreRules(content)).toEqual(['a/', 'b/', 'c/'])
  })

  it('trims whitespace', () => {
    const content = '  node_modules/  \n  dist/  '
    expect(parseGitGovIgnoreRules(content)).toEqual(['node_modules/', 'dist/'])
  })
})

describe('matchesGitGovIgnoreRule', () => {
  it('matches directory rule anywhere in path', () => {
    expect(matchesGitGovIgnoreRule('node_modules/react/index.js', 'node_modules/')).toBe(true)
    expect(matchesGitGovIgnoreRule('packages/app/node_modules/x', 'node_modules/')).toBe(true)
  })

  it('matches exact directory path rule', () => {
    expect(matchesGitGovIgnoreRule('app/.next/cache/x', 'app/.next/')).toBe(true)
    expect(matchesGitGovIgnoreRule('other/.next/cache/x', 'app/.next/')).toBe(false)
  })

  it('matches glob patterns with *', () => {
    expect(matchesGitGovIgnoreRule('debug.log', '*.log')).toBe(true)
    expect(matchesGitGovIgnoreRule('nested/debug.log', '*.log')).toBe(true)
    expect(matchesGitGovIgnoreRule('debug.txt', '*.log')).toBe(false)
  })

  it('matches glob patterns with ?', () => {
    expect(matchesGitGovIgnoreRule('a.ts', '?.ts')).toBe(true)
    expect(matchesGitGovIgnoreRule('ab.ts', '?.ts')).toBe(false)
  })

  it('matches character class [cod]', () => {
    expect(matchesGitGovIgnoreRule('file.pyc', '*.py[cod]')).toBe(true)
    expect(matchesGitGovIgnoreRule('file.pyo', '*.py[cod]')).toBe(true)
    expect(matchesGitGovIgnoreRule('file.pyd', '*.py[cod]')).toBe(true)
    expect(matchesGitGovIgnoreRule('file.pyx', '*.py[cod]')).toBe(false)
  })

  it('returns false for empty rule', () => {
    expect(matchesGitGovIgnoreRule('anything', '')).toBe(false)
    expect(matchesGitGovIgnoreRule('anything', '   ')).toBe(false)
  })

  it('handles backslash paths', () => {
    expect(matchesGitGovIgnoreRule('node_modules\\react\\index.js', 'node_modules/')).toBe(true)
  })

  it('matches path with slash as full path match', () => {
    expect(matchesGitGovIgnoreRule('src/dist/output.js', 'src/dist/')).toBe(true)
    expect(matchesGitGovIgnoreRule('other/dist/output.js', 'src/dist/')).toBe(false)
  })
})

describe('firstMatchingGitGovIgnoreRule', () => {
  it('returns first matching rule', () => {
    const rules = ['dist/', 'node_modules/', 'build/']
    expect(firstMatchingGitGovIgnoreRule('node_modules/x', rules)).toBe('node_modules/')
  })

  it('returns null when no rule matches', () => {
    const rules = ['dist/', 'node_modules/']
    expect(firstMatchingGitGovIgnoreRule('src/main.ts', rules)).toBeNull()
  })
})

describe('isHiddenByGitGovIgnore', () => {
  it('returns true for hidden paths', () => {
    expect(isHiddenByGitGovIgnore('node_modules/react', ['node_modules/'])).toBe(true)
  })

  it('returns false for non-hidden paths', () => {
    expect(isHiddenByGitGovIgnore('src/app.ts', ['node_modules/'])).toBe(false)
  })
})

describe('analyzeLargeChangeset', () => {
  it('counts total and candidate files', () => {
    const files: FileChange[] = [
      fc('src/app.ts'),
      fc('node_modules/react/index.js'),
      fc('node_modules/react/package.json'),
      fc('dist/bundle.js'),
    ]
    const result = analyzeLargeChangeset(files)
    expect(result.totalFiles).toBe(4)
    expect(result.candidateFiles).toBe(3) // node_modules x2 + dist x1
  })

  it('generates rules sorted by count descending', () => {
    const files: FileChange[] = [
      fc('node_modules/a/x'),
      fc('node_modules/b/y'),
      fc('node_modules/c/z'),
      fc('dist/out.js'),
    ]
    const result = analyzeLargeChangeset(files)
    expect(result.rules.length).toBeGreaterThanOrEqual(2)
    expect(result.rules[0].rule).toBe('node_modules/')
    expect(result.rules[0].count).toBe(3)
  })

  it('identifies top-level folders', () => {
    const files: FileChange[] = [
      fc('src/a.ts'),
      fc('src/b.ts'),
      fc('lib/c.ts'),
    ]
    const result = analyzeLargeChangeset(files)
    expect(result.topFolders[0].folder).toBe('src')
    expect(result.topFolders[0].count).toBe(2)
  })

  it('detects stack templates from file markers', () => {
    const files: FileChange[] = [
      fc('package.json'),
      fc('src/index.ts'),
      fc('node_modules/react/index.js'),
    ]
    const result = analyzeLargeChangeset(files)
    const nodeTemplate = result.stackTemplates.find((t) => t.id === 'node')
    expect(nodeTemplate).toBeDefined()
    expect(nodeTemplate!.reasons.length).toBeGreaterThan(0)
  })

  it('detects Rust stack from Cargo.toml', () => {
    const files: FileChange[] = [
      fc('Cargo.toml'),
      fc('src/main.rs'),
      fc('target/debug/binary'),
    ]
    const result = analyzeLargeChangeset(files)
    const rustTemplate = result.stackTemplates.find((t) => t.id === 'rust')
    expect(rustTemplate).toBeDefined()
  })

  it('detects Python stack from pyproject.toml', () => {
    const files: FileChange[] = [
      fc('pyproject.toml'),
      fc('src/main.py'),
      fc('__pycache__/module.pyc'),
    ]
    const result = analyzeLargeChangeset(files)
    const pyTemplate = result.stackTemplates.find((t) => t.id === 'python')
    expect(pyTemplate).toBeDefined()
  })

  it('detects Java stack from pom.xml', () => {
    const files: FileChange[] = [
      fc('pom.xml'),
      fc('src/main/java/App.java'),
    ]
    const result = analyzeLargeChangeset(files)
    const javaTemplate = result.stackTemplates.find((t) => t.id === 'java')
    expect(javaTemplate).toBeDefined()
  })

  it('detects Next.js stack from next.config.js', () => {
    const files: FileChange[] = [
      fc('next.config.js'),
      fc('package.json'),
      fc('.next/cache/webpack/hot.js'),
    ]
    const result = analyzeLargeChangeset(files)
    const nextTemplate = result.stackTemplates.find((t) => t.id === 'nextjs')
    expect(nextTemplate).toBeDefined()
  })

  it('returns empty analysis for empty file list', () => {
    const result = analyzeLargeChangeset([])
    expect(result.totalFiles).toBe(0)
    expect(result.candidateFiles).toBe(0)
    expect(result.rules).toEqual([])
    expect(result.stackTemplates).toEqual([])
  })

  it('collects matched segments', () => {
    const files: FileChange[] = [
      fc('node_modules/x'),
      fc('.cache/y'),
      fc('__pycache__/z'),
    ]
    const result = analyzeLargeChangeset(files)
    expect(result.matchedSegments).toContain('node_modules')
    expect(result.matchedSegments).toContain('.cache')
    expect(result.matchedSegments).toContain('__pycache__')
    // Should be sorted
    expect(result.matchedSegments).toEqual([...result.matchedSegments].sort())
  })
})
