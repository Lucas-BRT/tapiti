// One worker of the lab's pool: its own instance of the module (own memory and allocator). It is asked for slices of a batch, or for one
// floor (the page keeps drawing while it is made). The module lives at a fixed path, outside the hashed assets, because the glue finds
// its `.wasm` next to itself.
import init, { bench, stage_json } from '/lab/pkg/tapiti.js'

const ready = init()

self.onmessage = async ({ data }) => {
  await ready
  if (data.type === 'stage') {
    try {
      self.postMessage({ id: data.id, json: stage_json(data.seed, data.params) })
    } catch (error) {
      self.postMessage({ id: data.id, error: String(error?.message ?? error) })
    }
    return
  }
  const { id, size, from, count } = data
  self.postMessage({ id, ...JSON.parse(bench(size, from, count)) })
}
