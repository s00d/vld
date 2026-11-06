// Copies CHANGELOG.md and MSRV.md from the repository root into content/ (both gitignored),
// adds frontmatter and points links to other repository files at GitHub.
import { readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const root = new URL('../../', import.meta.url)
const content = new URL('../content/', import.meta.url)
const repo = 'https://github.com/s00d/vld/blob/main/'

const pages = [
  {
    from: 'CHANGELOG.md',
    to: '9.changelog.md',
    frontmatter: { title: 'Changelog', description: 'All notable changes to vld and its integration crates.' },
  },
  {
    from: 'MSRV.md',
    to: '1.getting-started/9.msrv.md',
    frontmatter: { title: 'MSRV', description: 'Minimum supported Rust version of every crate and feature.' },
  },
]

for (const page of pages) {
  let body = readFileSync(new URL(page.from, root), 'utf8')
  // The page title comes from frontmatter; drop the file's own `# Heading`.
  body = body.replace(/^# .*\n/, '')
  // `[MSRV.md](MSRV.md)` and similar relative links would 404 on the site.
  body = body.replace(/\]\((?!https?:|#|\/)([^)]+)\)/g, (_, target) => {
    if (target === 'MSRV.md') return '](/getting-started/msrv)'
    if (target === 'CHANGELOG.md') return '](/changelog)'
    return `](${repo}${target})`
  })
  const fm = Object.entries(page.frontmatter)
    .map(([key, value]) => `${key}: ${JSON.stringify(value)}`)
    .join('\n')
  writeFileSync(new URL(page.to, content), `---\n${fm}\n---\n\n<!-- Generated from ${page.from} by docs/scripts/sync.mjs. Edit the original. -->\n\n${body}`)
  console.log(`synced ${page.from} -> ${fileURLToPath(new URL(page.to, content))}`)
}
