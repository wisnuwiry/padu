import type { ProviderKind } from '@padu/client'

export const PADU_ICONS = {
  addToChat: 'i-padu-add-to-chat',
  alert: 'i-padu-alert',
  appearance: 'i-padu-appearance',
  archive: 'i-padu-archive',
  arrowDown: 'i-padu-arrow-down',
  arrowLeft: 'i-padu-arrow-left',
  arrowRight: 'i-padu-arrow-right',
  arrowUp: 'i-padu-arrow-up',
  arrowUpRight: 'i-padu-arrow-up-right',
  bell: 'i-padu-bell',
  bot: 'i-padu-bot',
  chartColumn: 'i-padu-chart-column',
  check: 'i-padu-check',
  chevronDown: 'i-padu-chevron-down',
  chevronRight: 'i-padu-chevron-right',
  chevronUp: 'i-padu-chevron-up',
  chevronsDownUp: 'i-padu-chevrons-down-up',
  chevronsUpDown: 'i-padu-chevrons-up-down',
  cloudUpload: 'i-padu-cloud-upload',
  collapse2: 'i-padu-collapse-2',
  command: 'i-padu-command',
  compose: 'i-padu-compose',
  copy: 'i-padu-copy',
  cornerDownLeft: 'i-padu-corner-down-left',
  cornerDownRight: 'i-padu-corner-down-right',
  ellipsis: 'i-padu-ellipsis',
  expand2: 'i-padu-expand-2',
  eye: 'i-padu-eye',
  eyeOff: 'i-padu-eye-off',
  file: 'i-padu-file',
  fileAdd: 'i-padu-file-add',
  fileDiff: 'i-padu-file-diff',
  folder: 'i-padu-folder',
  folderNew: 'i-padu-folder-new',
  folderOpen: 'i-padu-folder-open',
  fork: 'i-padu-fork',
  gauge: 'i-padu-gauge',
  gitBranch: 'i-padu-git-branch',
  gitCommitHorizontal: 'i-padu-git-commit-horizontal',
  globe: 'i-padu-globe',
  github: 'i-padu-github',
  heart: 'i-padu-heart',
  hierarchyFiles: 'i-padu-hierarchy-files',
  info: 'i-padu-info',
  logo: 'i-padu-logo',
  laptop: 'i-padu-laptop',
  list: 'i-padu-list',
  listFiles: 'i-padu-list-files',
  listFilter: 'i-padu-list-filter',
  loaderCircle: 'i-padu-loader-circle',
  lock: 'i-padu-lock',
  lockOpen: 'i-padu-lock-open',
  note: 'i-padu-note',
  noteAdd: 'i-padu-note-add',
  package: 'i-padu-package',
  paperclip: 'i-padu-paperclip',
  panelLeft: 'i-padu-panel-left',
  panelRight: 'i-padu-panel-right',
  pin: 'i-padu-pin',
  pencil: 'i-padu-pencil',
  plus: 'i-padu-plus',
  queue: 'i-padu-queue',
  rotateCw: 'i-padu-rotate-cw',
  rewind: 'i-padu-rewind',
  search: 'i-padu-search',
  server: 'i-padu-server',
  settings: 'i-padu-settings',
  split: 'i-padu-split',
  sparkle: 'i-padu-sparkle',
  star: 'i-padu-star',
  starFilled: 'i-padu-star-filled',
  stop: 'i-padu-stop',
  target: 'i-padu-target',
  stopFilled: 'i-padu-stop-filled',
  terminal: 'i-padu-terminal',
  terminalSquare: 'i-padu-terminal-square',
  trash: 'i-padu-trash',
  maximize: 'i-padu-maximize',
  minimize: 'i-padu-minimize',
  windowMaximize: 'i-padu-window-maximize',
  windowMinimize: 'i-padu-window-minimize',
  wrench: 'i-padu-wrench',
  x: 'i-padu-x',
  zap: 'i-padu-zap',
} as const

export type PaduIconName = keyof typeof PADU_ICONS

export function PaduIcon({
  name,
  className,
  label,
}: {
  name: PaduIconName
  className?: string
  label?: string
}) {
  return (
    <span
      aria-hidden={label ? undefined : true}
      aria-label={label}
      className={`inline-grid size-4 shrink-0 place-items-center ${className ?? ''}`}
      role={label ? 'img' : undefined}
    >
      <span
        aria-hidden="true"
        className={PADU_ICONS[name]}
        style={{ width: '100%', height: '100%' }}
      />
    </span>
  )
}

const FILE_TYPE_ICONS = {
  angular: 'i-padu-file-type-angular',
  astro: 'i-padu-file-type-astro',
  audio: 'i-padu-file-type-audio',
  babel: 'i-padu-file-type-babel',
  biome: 'i-padu-file-type-biome',
  bun: 'i-padu-file-type-bun',
  c: 'i-padu-file-type-c',
  certificate: 'i-padu-file-type-certificate',
  clojure: 'i-padu-file-type-clojure',
  cmake: 'i-padu-file-type-cmake',
  coffee: 'i-padu-file-type-coffee',
  console: 'i-padu-file-type-console',
  cpp: 'i-padu-file-type-cpp',
  crystal: 'i-padu-file-type-crystal',
  csharp: 'i-padu-file-type-csharp',
  css: 'i-padu-file-type-css',
  dart: 'i-padu-file-type-dart',
  database: 'i-padu-file-type-database',
  deno: 'i-padu-file-type-deno',
  diff: 'i-padu-file-type-diff',
  docker: 'i-padu-file-type-docker',
  editorconfig: 'i-padu-file-type-editorconfig',
  elixir: 'i-padu-file-type-elixir',
  elm: 'i-padu-file-type-elm',
  erlang: 'i-padu-file-type-erlang',
  eslint: 'i-padu-file-type-eslint',
  exe: 'i-padu-file-type-exe',
  file: 'i-padu-file-type-file',
  firebase: 'i-padu-file-type-firebase',
  git: 'i-padu-file-type-git',
  gitlab: 'i-padu-file-type-gitlab',
  go: 'i-padu-file-type-go',
  gradle: 'i-padu-file-type-gradle',
  graphql: 'i-padu-file-type-graphql',
  haskell: 'i-padu-file-type-haskell',
  haxe: 'i-padu-file-type-haxe',
  helm: 'i-padu-file-type-helm',
  html: 'i-padu-file-type-html',
  image: 'i-padu-file-type-image',
  java: 'i-padu-file-type-java',
  javascript: 'i-padu-file-type-javascript',
  jinja: 'i-padu-file-type-jinja',
  json: 'i-padu-file-type-json',
  julia: 'i-padu-file-type-julia',
  kotlin: 'i-padu-file-type-kotlin',
  kubernetes: 'i-padu-file-type-kubernetes',
  lock: 'i-padu-file-type-lock',
  lua: 'i-padu-file-type-lua',
  makefile: 'i-padu-file-type-makefile',
  markdown: 'i-padu-file-type-markdown',
  nest: 'i-padu-file-type-nest',
  next: 'i-padu-file-type-next',
  nginx: 'i-padu-file-type-nginx',
  nix: 'i-padu-file-type-nix',
  nodejs: 'i-padu-file-type-nodejs',
  npm: 'i-padu-file-type-npm',
  nuxt: 'i-padu-file-type-nuxt',
  ocaml: 'i-padu-file-type-ocaml',
  pdf: 'i-padu-file-type-pdf',
  perl: 'i-padu-file-type-perl',
  php: 'i-padu-file-type-php',
  pnpm: 'i-padu-file-type-pnpm',
  powershell: 'i-padu-file-type-powershell',
  prettier: 'i-padu-file-type-prettier',
  prisma: 'i-padu-file-type-prisma',
  proto: 'i-padu-file-type-proto',
  pug: 'i-padu-file-type-pug',
  python: 'i-padu-file-type-python',
  react: 'i-padu-file-type-react',
  readme: 'i-padu-file-type-readme',
  rollup: 'i-padu-file-type-rollup',
  ruby: 'i-padu-file-type-ruby',
  rust: 'i-padu-file-type-rust',
  sass: 'i-padu-file-type-sass',
  scala: 'i-padu-file-type-scala',
  settings: 'i-padu-file-type-settings',
  solidity: 'i-padu-file-type-solidity',
  storybook: 'i-padu-file-type-storybook',
  stylelint: 'i-padu-file-type-stylelint',
  supabase: 'i-padu-file-type-supabase',
  svelte: 'i-padu-file-type-svelte',
  svg: 'i-padu-file-type-svg',
  swift: 'i-padu-file-type-swift',
  tailwindcss: 'i-padu-file-type-tailwindcss',
  terraform: 'i-padu-file-type-terraform',
  tex: 'i-padu-file-type-tex',
  turborepo: 'i-padu-file-type-turborepo',
  typescript: 'i-padu-file-type-typescript',
  video: 'i-padu-file-type-video',
  vite: 'i-padu-file-type-vite',
  vitest: 'i-padu-file-type-vitest',
  vue: 'i-padu-file-type-vue',
  webassembly: 'i-padu-file-type-webassembly',
  webpack: 'i-padu-file-type-webpack',
  xaml: 'i-padu-file-type-xaml',
  xml: 'i-padu-file-type-xml',
  yaml: 'i-padu-file-type-yaml',
  yarn: 'i-padu-file-type-yarn',
  zig: 'i-padu-file-type-zig',
  zip: 'i-padu-file-type-zip',
} as const

type FileTypeIconName = keyof typeof FILE_TYPE_ICONS

export function FileTypeIcon({
  path,
  className,
}: {
  path: string
  className?: string
}) {
  const name = fileTypeIconName(path)
  return (
    <span
      aria-hidden="true"
      className={`inline-grid size-4 shrink-0 place-items-center ${className ?? ''}`}
    >
      <span className={FILE_TYPE_ICONS[name]} style={{ width: '100%', height: '100%' }} />
    </span>
  )
}

export function fileTypeIconClass(path: string): string {
  return FILE_TYPE_ICONS[fileTypeIconName(path)]
}

function fileTypeIconName(path: string): FileTypeIconName {
  const name = path.split(/[\\/]/).at(-1)?.toLocaleLowerCase() ?? path.toLocaleLowerCase()
  if (name.startsWith('readme')) return 'readme'
  if (/^(license|licence|copying)/.test(name)) return 'certificate'
  if (name.startsWith('dockerfile') || name.startsWith('compose.')) return 'docker'
  if (name === 'cmakelists.txt' || name.startsWith('cmake.')) return 'cmake'
  if (name === 'makefile' || name.startsWith('makefile.') || name === 'justfile') return 'makefile'
  if (['cargo.toml', 'cargo.lock', 'rust-toolchain.toml'].includes(name)) return 'rust'
  if (['go.mod', 'go.sum', 'go.work'].includes(name)) return 'go'
  if (name === 'pyproject.toml' || name === 'pipfile' || name.startsWith('requirements')) return 'python'
  if (['bun.lock', 'bun.lockb', 'bunfig.toml'].includes(name)) return 'bun'
  if (name.startsWith('pnpm-') || name === '.pnpmfile.cjs') return 'pnpm'
  if (name === 'yarn.lock' || name.startsWith('.yarnrc')) return 'yarn'
  if (name === 'package.json') return 'nodejs'
  if (name === 'package-lock.json') return 'npm'
  if (name === 'tsconfig.json' || name.startsWith('tsconfig.')) return 'typescript'
  if (name === 'jsconfig.json' || name.startsWith('jsconfig.')) return 'javascript'
  if (['.gitignore', '.gitattributes', '.gitmodules', '.gitconfig'].includes(name)) return 'git'
  if (name === '.editorconfig') return 'editorconfig'
  if (name.startsWith('.env')) return 'settings'
  if (name.startsWith('.prettier') || name.startsWith('prettier.config.')) return 'prettier'
  if (name.startsWith('.eslint') || name.startsWith('eslint.config.')) return 'eslint'
  if (name.startsWith('biome.json')) return 'biome'
  if (name.startsWith('.babel') || name.startsWith('babel.config.')) return 'babel'
  if (name.startsWith('.stylelint') || name.startsWith('stylelint.config.')) return 'stylelint'
  if (name.startsWith('vite.config.')) return 'vite'
  if (name.startsWith('vitest.config.') || name.startsWith('vitest.workspace.')) return 'vitest'
  if (name.startsWith('webpack.')) return 'webpack'
  if (name.startsWith('rollup.config.')) return 'rollup'
  if (name.startsWith('next.config.') || name === 'next-env.d.ts') return 'next'
  if (name.startsWith('nuxt.config.') || name === '.nuxtrc') return 'nuxt'
  if (name.startsWith('astro.config.')) return 'astro'
  if (name === 'angular.json' || name.endsWith('.component.ts')) return 'angular'
  if (name === 'nest-cli.json') return 'nest'
  if (name.startsWith('tailwind.config.')) return 'tailwindcss'
  if (name.startsWith('svelte.config.')) return 'svelte'
  if (name.startsWith('vue.config.')) return 'vue'
  if (name === 'firebase.json' || name === '.firebaserc') return 'firebase'
  if (name === 'supabase.toml') return 'supabase'
  if (name.startsWith('prisma.config.')) return 'prisma'
  if (name === 'turbo.json') return 'turborepo'
  if (name.startsWith('deno.json') || name === 'deno.lock') return 'deno'
  if (name === '.gitlab-ci.yml' || name === '.gitlab-ci.yaml') return 'gitlab'
  if (name === 'kustomization.yaml' || name === 'kustomization.yml') return 'kubernetes'
  if (name === 'chart.yaml' || name === 'values.yaml') return 'helm'
  if (name === 'nginx.conf') return 'nginx'
  if (name === '.nvmrc' || name === '.node-version') return 'nodejs'
  if (['build.gradle', 'settings.gradle', 'gradlew', 'gradlew.bat'].includes(name)) return 'gradle'
  if (name.includes('.stories.') || name.includes('.story.')) return 'storybook'
  if (name === 'gemfile' || name === 'gemfile.lock') return 'ruby'
  if (name === 'pom.xml') return 'java'

  const extension = name.includes('.') ? name.split('.').at(-1) ?? '' : ''
  if (extension === 'rs') return 'rust'
  if (['js', 'mjs', 'cjs'].includes(extension)) return 'javascript'
  if (['ts', 'mts', 'cts'].includes(extension)) return 'typescript'
  if (['jsx', 'tsx'].includes(extension)) return 'react'
  if (['py', 'pyi', 'pyw'].includes(extension)) return 'python'
  if (extension === 'go') return 'go'
  if (['c', 'h', 'm'].includes(extension)) return 'c'
  if (['cc', 'cpp', 'cxx', 'hh', 'hpp', 'hxx', 'mm'].includes(extension)) return 'cpp'
  if (extension === 'cs') return 'csharp'
  if (extension === 'swift') return 'swift'
  if (['kt', 'kts'].includes(extension)) return 'kotlin'
  if (['java', 'class'].includes(extension)) return 'java'
  if (extension === 'rb') return 'ruby'
  if (extension === 'php') return 'php'
  if (['html', 'htm'].includes(extension)) return 'html'
  if (['css', 'less'].includes(extension)) return 'css'
  if (['scss', 'sass'].includes(extension)) return 'sass'
  if (['json', 'jsonc', 'jsonl'].includes(extension)) return 'json'
  if (['yaml', 'yml'].includes(extension)) return 'yaml'
  if (['toml', 'ini', 'cfg', 'conf', 'config'].includes(extension)) return 'settings'
  if (['xml', 'xsl', 'plist'].includes(extension)) return 'xml'
  if (['md', 'mdx', 'markdown'].includes(extension)) return 'markdown'
  if (['sh', 'bash', 'zsh', 'fish'].includes(extension)) return 'console'
  if (['ps1', 'psm1'].includes(extension)) return 'powershell'
  if (['sql', 'db', 'sqlite', 'sqlite3', 'csv', 'xls', 'xlsx'].includes(extension)) return 'database'
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'avif', 'ico', 'tiff'].includes(extension)) return 'image'
  if (extension === 'svg') return 'svg'
  if (extension === 'pdf') return 'pdf'
  if (['mp3', 'wav', 'flac', 'ogg', 'm4a'].includes(extension)) return 'audio'
  if (['mp4', 'mov', 'avi', 'webm', 'mkv'].includes(extension)) return 'video'
  if (['zip', 'gz', 'tgz', 'bz2', 'xz', '7z', 'rar', 'tar', 'jar'].includes(extension)) return 'zip'
  if (['wasm', 'wat'].includes(extension)) return 'webassembly'
  if (['svelte', 'vue', 'lua', 'dart', 'astro', 'prisma', 'xaml', 'zig', 'nix', 'proto'].includes(extension)) return extension as FileTypeIconName
  if (['tf', 'tfvars'].includes(extension)) return 'terraform'
  if (['graphql', 'gql'].includes(extension)) return 'graphql'
  if (['coffee', 'cson'].includes(extension)) return 'coffee'
  if (extension === 'cr') return 'crystal'
  if (['ex', 'exs'].includes(extension)) return 'elixir'
  if (extension === 'elm') return 'elm'
  if (['erl', 'hrl'].includes(extension)) return 'erlang'
  if (['clj', 'cljs', 'cljc', 'edn'].includes(extension)) return 'clojure'
  if (['hs', 'lhs'].includes(extension)) return 'haskell'
  if (['hx', 'hxml'].includes(extension)) return 'haxe'
  if (['jinja', 'jinja2', 'j2'].includes(extension)) return 'jinja'
  if (extension === 'jl') return 'julia'
  if (['ml', 'mli'].includes(extension)) return 'ocaml'
  if (['pl', 'pm'].includes(extension)) return 'perl'
  if (['pug', 'jade'].includes(extension)) return 'pug'
  if (['scala', 'sbt', 'sc'].includes(extension)) return 'scala'
  if (extension === 'sol') return 'solidity'
  if (['tex', 'sty', 'cls'].includes(extension)) return 'tex'
  if (['diff', 'patch'].includes(extension)) return 'diff'
  if (['exe', 'dll', 'so', 'dylib'].includes(extension)) return 'exe'
  if (extension === 'lock') return 'lock'
  return 'file'
}

const PROVIDER_ICONS: Record<ProviderKind, string> = {
  agy: 'i-padu-provider-agy',
  amp: 'i-padu-provider-amp',
  claude: 'i-padu-provider-claude',
  codex: 'i-padu-provider-openai',
  cursor: 'i-padu-provider-cursor',
  deepSeek: 'i-padu-provider-deepseek',
  fx: 'i-padu-provider-fx',
  openCode: 'i-padu-provider-opencode',
  grok: 'i-padu-provider-grok',
  kimi: 'i-padu-provider-kimi',
  ohMyPi: 'i-padu-provider-ohmypi',
  pi: 'i-padu-provider-pi',
}

export const PROVIDERS: Array<{
  id: ProviderKind
  name: string
  shortName: string
  command: string
}> = [
  { id: 'agy', name: 'Antigravity', shortName: 'Antigravity', command: 'agy_acp_server.par' },
  { id: 'amp', name: 'Amp', shortName: 'Amp', command: 'amp' },
  { id: 'claude', name: 'Claude Code', shortName: 'Claude', command: 'claude' },
  { id: 'codex', name: 'Codex CLI', shortName: 'Codex', command: 'codex' },
  { id: 'cursor', name: 'Cursor CLI', shortName: 'Cursor', command: 'cursor-agent' },
  { id: 'deepSeek', name: 'DeepSeek Harness', shortName: 'DeepSeek', command: 'dsh' },
  { id: 'fx', name: 'Fx', shortName: 'Fx', command: 'fx' },
  { id: 'openCode', name: 'OpenCode', shortName: 'OpenCode', command: 'opencode' },
  { id: 'grok', name: 'Grok Build', shortName: 'Grok', command: 'grok' },
  { id: 'kimi', name: 'Kimi Code', shortName: 'Kimi', command: 'kimi' },
  { id: 'ohMyPi', name: 'Oh My Pi', shortName: 'Oh My Pi', command: 'omp' },
  { id: 'pi', name: 'Pi', shortName: 'Pi', command: 'pi' },
]

export function providerMeta(provider: ProviderKind) {
  return PROVIDERS.find((candidate) => candidate.id === provider) ?? PROVIDERS[2]!
}

export function ProviderIcon({
  provider,
  className,
  label,
}: {
  provider: ProviderKind
  className?: string
  label?: string
}) {
  return (
    <span
      aria-hidden={label ? undefined : true}
      aria-label={label}
      className={`inline-grid size-4 shrink-0 place-items-center ${providerColor(provider)} ${className ?? ''}`}
      role={label ? 'img' : undefined}
    >
      <span
        aria-hidden="true"
        className={PROVIDER_ICONS[provider]}
        style={{ width: '100%', height: '100%' }}
      />
    </span>
  )
}

function providerColor(provider: ProviderKind) {
  if (provider === 'amp') return 'text-[#f34e3f]'
  if (provider === 'claude') return 'text-[#d97757]'
  if (provider === 'deepSeek') return 'text-[#4d6bfe]'
  return 'text-[#34363b] dark:text-[#f3f3f3]'
}
