// Static/synthetic audit only. Never reads real environment credentials or user data.
import { readFileSync, readdirSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { resolve, join, relative } from 'node:path'
import assert from 'node:assert/strict'

const root = resolve(import.meta.dirname, '..')
const walk = path => readdirSync(path, { withFileTypes: true }).flatMap(entry => entry.isDirectory() ? walk(join(path, entry.name)) : [join(path, entry.name)])
const bundle = walk(join(root, 'frontend/dist')).filter(path => /\.(js|css|html)$/.test(path))
assert(bundle.length > 0, 'Build frontend first')
for (const file of bundle) {
  const content = readFileSync(file, 'utf8')
  assert(!/127\.0\.0\.1|localhost|:5174\b/.test(content), `Local endpoint in ${relative(root, file)}`)
  assert(!content.includes('MELODYPATH_SYNTHETIC_SECRET_CANARY_'), `Synthetic backend secret leaked into ${relative(root,file)}`)
  assert(!/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|AIza[0-9A-Za-z_-]{35}|ghp_[0-9A-Za-z]{36}/.test(content), `Potential credential in ${relative(root,file)}`)
}
const api = readFileSync(join(root,'frontend/src/api.ts'),'utf8')
assert(api.includes("fetch('/api/session'"))
assert(!/https?:\/\//.test(api), 'Browser API must stay relative')
const files = execFileSync('git',['ls-files','-z'],{cwd:root,encoding:'utf8'}).split('\0').filter(Boolean)
for (const file of files) {
  // Inspect names before reading; never open credential, data or private playlist files.
  assert(!/(^|\/)\.env$|(^|\/)\.env\.(?!example$)|\.db(?:-wal|-shm)?$|\.pem$|\.p8$|client_secret.*\.json$|oauth-.*\.bin$/.test(file), `Sensitive file is tracked: ${file}`)
  if (!/\.(rs|tsx?|md|json|ya?ml|ps1|sh)$/.test(file)) continue
  const content = readFileSync(join(root,file),'utf8')
  assert(!/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|AIza[0-9A-Za-z_-]{35}|ghp_[0-9A-Za-z]{36}/.test(content), `Potential credential in tracked file: ${file}`)
}
console.log(`PASS: ${bundle.length} production assets use no loopback endpoints or secret canaries; tracked-file credential patterns clean.`)
console.log('This pattern/canary audit cannot prove the absence of every possible secret format.')
