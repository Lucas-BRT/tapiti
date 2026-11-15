// Compares the native build with the WebAssembly module: for every line of native.jsonl (the Λ and the native answer, see
// crates/tapiti-core/examples/parity.rs) the module's `stage_json` must answer the same floor, violations, draws and measures,
// number for number. Usage: node scripts/wasm-parity.mjs <dir with native.jsonl and the nodejs build of the module>
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'

const dir = resolve(process.argv[2] ?? 'target/wasm-parity')
const wasm = createRequire(import.meta.url)(resolve(dir, 'tapiti.js'))
const lines = readFileSync(resolve(dir, 'native.jsonl'), 'utf8').trim().split('\n')
let failed = 0
for (const line of lines) {
  const { name, seed, params, answer } = JSON.parse(line)
  const got = JSON.parse(wasm.stage_json(seed, JSON.stringify(params)))
  delete got.ms
  delete got.floor.phases
  try {
    assert.deepStrictEqual(got, answer)
    console.log(`ok   ${name} seed ${seed}`)
  } catch (error) {
    failed++
    const first = String(error.message).split('\n').slice(0, 12).join('\n')
    console.log(`FAIL ${name} seed ${seed}\n${first}`)
  }
}
console.log(`${lines.length - failed} of ${lines.length} cases match`)

// The 3D look: the fingerprint of the mesh and of two pictures, native against the module.
const views = readFileSync(resolve(dir, 'native-view.jsonl'), 'utf8').trim().split('\n')
let viewFailed = 0
for (const line of views) {
  const { name, seed, params, digest } = JSON.parse(line)
  const got = wasm.view_digest(seed, JSON.stringify(params))
  if (got === digest) {
    console.log(`ok   view ${name} seed ${seed}`)
  } else {
    viewFailed++
    console.log(`FAIL view ${name} seed ${seed}\n  native ${digest}\n  module ${got}`)
  }
}
console.log(`${views.length - viewFailed} of ${views.length} 3D fingerprints match`)
process.exit(failed || viewFailed ? 1 : 0)
