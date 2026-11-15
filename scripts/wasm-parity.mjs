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
process.exit(failed ? 1 : 0)
