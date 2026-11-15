// Geometry of the walk, from the floor the generator returns. Pure data: floor JSON in, typed arrays out. No three.js, no DOM,
// no randomness, and only + - * / sqrt, so every browser (and Node) builds the same bytes, and the scene can be checked without a GPU.
//
// World units are the floor's own (pixels). A floor point (x, y) is the three.js point (x, 0, y); walls rise along +y.
//
//   floor ──► signed field on a 4-unit grid (rooms and corridor segments, union by max)
//         ──► marching squares (linear interpolation) ──► closed contours ──► Douglas-Peucker ──► wall quads
//         ──► torches along room walls, corner pillars at junctions, the camera path along the main path, the stations

export const CELL = 4
export const WALL_HEIGHT = 60
export const FOOT = 8
export const SIMPLIFY = 1.5
export const STATIONS = 7

const BUCKET = 64
const FAR = -16

const sqrt = Math.sqrt

/* ---------------- Signed values: positive inside a shape, negative outside, about the distance to its outline ---------------- */

function pointSegment(px, py, ax, ay, bx, by) {
  const dx = bx - ax
  const dy = by - ay
  const len2 = dx * dx + dy * dy
  let t = len2 ? ((px - ax) * dx + (py - ay) * dy) / len2 : 0
  t = t < 0 ? 0 : t > 1 ? 1 : t
  const qx = ax + t * dx - px
  const qy = ay + t * dy - py
  return sqrt(qx * qx + qy * qy)
}

function polygonValue(pts, x, y) {
  let inside = false
  let nearest = Infinity
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [xi, yi] = pts[i]
    const [xj, yj] = pts[j]
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside
    const d = pointSegment(x, y, xj, yj, xi, yi)
    if (d < nearest) nearest = d
  }
  return inside ? nearest : -nearest
}

function shapeValue(shape, x, y) {
  if (shape.kind === 'disc') {
    const dx = x - shape.c[0]
    const dy = y - shape.c[1]
    return shape.r - sqrt(dx * dx + dy * dy)
  }
  return polygonValue(shape.pts, x, y)
}

/** Items of the field: every alive room, and every corridor segment (a stroke of the corridor's width). */
function fieldItems(floor) {
  const half = floor.params.width / 2
  const items = []
  for (const room of floor.rooms) {
    if (!room.alive) continue
    const s = room.shape
    const box =
      s.kind === 'disc'
        ? [s.c[0] - s.r, s.c[1] - s.r, s.c[0] + s.r, s.c[1] + s.r]
        : s.pts.reduce((b, [x, y]) => [Math.min(b[0], x), Math.min(b[1], y), Math.max(b[2], x), Math.max(b[3], y)], [Infinity, Infinity, -Infinity, -Infinity])
    items.push({ room, box })
  }
  for (const edge of floor.edges) {
    for (let i = 1; i < edge.pts.length; i++) {
      const [ax, ay] = edge.pts[i - 1]
      const [bx, by] = edge.pts[i]
      items.push({ seg: [ax, ay, bx, by], half, box: [Math.min(ax, bx) - half, Math.min(ay, by) - half, Math.max(ax, bx) + half, Math.max(ay, by) + half] })
    }
  }
  return items
}

function bucketed(items, margin) {
  const map = new Map()
  const key = (bx, by) => bx + by * 4096
  for (const item of items) {
    const x0 = Math.floor((item.box[0] - margin) / BUCKET)
    const y0 = Math.floor((item.box[1] - margin) / BUCKET)
    const x1 = Math.floor((item.box[2] + margin) / BUCKET)
    const y1 = Math.floor((item.box[3] + margin) / BUCKET)
    for (let by = y0; by <= y1; by++) {
      for (let bx = x0; bx <= x1; bx++) {
        const k = key(bx, by)
        const list = map.get(k)
        if (list) list.push(item)
        else map.set(k, [item])
      }
    }
  }
  return (x, y) => map.get(key(Math.floor(x / BUCKET), Math.floor(y / BUCKET)))
}

function itemValue(item, x, y) {
  if (item.room) return shapeValue(item.room.shape, x, y)
  const [ax, ay, bx, by] = item.seg
  return item.half - pointSegment(x, y, ax, ay, bx, by)
}

/* ---------------- The field on a grid ---------------- */

function sampleField(floor) {
  const x0 = floor.gridX[0]
  const y0 = floor.gridY[0]
  const x1 = floor.gridX[floor.gridX.length - 1]
  const y1 = floor.gridY[floor.gridY.length - 1]
  const ox = x0 - 2 * CELL
  const oy = y0 - 2 * CELL
  const nx = Math.floor((x1 - x0) / CELL) + 5
  const ny = Math.floor((y1 - y0) / CELL) + 5
  const items = fieldItems(floor)
  const near = bucketed(items, 3 * CELL)
  const values = new Float32Array(nx * ny)
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      const x = ox + i * CELL
      const y = oy + j * CELL
      const list = near(x, y)
      let v = FAR
      if (list) for (const item of list) { const t = itemValue(item, x, y); if (t > v) v = t }
      values[j * nx + i] = v > -FAR ? -FAR : v
    }
  }
  return { nx, ny, ox, oy, values, bounds: [x0, y0, x1, y1], items }
}

/* ---------------- Marching squares ---------------- */

// Edge ids: H(i, j) joins samples (i, j) and (i+1, j); V(i, j) joins (i, j) and (i, j+1).
function contours(field) {
  const { nx, ny, ox, oy, values } = field
  const H = (i, j) => 2 * (j * nx + i)
  const V = (i, j) => 2 * (j * nx + i) + 1
  const crossing = (id) => {
    const vertical = id & 1
    const cell = id >> 1
    const j = Math.floor(cell / nx)
    const i = cell - j * nx
    const a = values[j * nx + i]
    const b = vertical ? values[(j + 1) * nx + i] : values[j * nx + i + 1]
    const t = a / (a - b)
    return vertical ? [ox + i * CELL, oy + (j + t) * CELL] : [ox + (i + t) * CELL, oy + j * CELL]
  }
  const segments = []
  for (let j = 0; j + 1 < ny; j++) {
    for (let i = 0; i + 1 < nx; i++) {
      const tl = values[j * nx + i]
      const tr = values[j * nx + i + 1]
      const br = values[(j + 1) * nx + i + 1]
      const bl = values[(j + 1) * nx + i]
      const code = (tl > 0 ? 1 : 0) | (tr > 0 ? 2 : 0) | (br > 0 ? 4 : 0) | (bl > 0 ? 8 : 0)
      if (code === 0 || code === 15) continue
      const T = H(i, j)
      const B = H(i, j + 1)
      const L = V(i, j)
      const R = V(i + 1, j)
      switch (code) {
        case 1: case 14: segments.push([L, T]); break
        case 2: case 13: segments.push([T, R]); break
        case 3: case 12: segments.push([L, R]); break
        case 4: case 11: segments.push([R, B]); break
        case 6: case 9: segments.push([T, B]); break
        case 7: case 8: segments.push([L, B]); break
        default: {
          const inside = (tl + tr + br + bl) / 4 > 0
          if (code === 5) segments.push(inside ? [T, R] : [L, T], inside ? [L, B] : [R, B])
          else segments.push(inside ? [L, T] : [T, R], inside ? [R, B] : [L, B])
        }
      }
    }
  }
  // Link the segments through the edges they share into closed loops.
  const byEdge = new Map()
  segments.forEach((s, n) => { for (const e of s) { const l = byEdge.get(e); if (l) l.push(n); else byEdge.set(e, [n]) } })
  const used = new Uint8Array(segments.length)
  const loops = []
  for (let first = 0; first < segments.length; first++) {
    if (used[first]) continue
    const ids = []
    let n = first
    let from = segments[first][0]
    for (;;) {
      used[n] = 1
      const [a, b] = segments[n]
      const to = a === from ? b : a
      ids.push(from)
      from = to
      const next = byEdge.get(to).find((m) => !used[m])
      if (next === undefined) break
      n = next
    }
    loops.push(ids.map(crossing))
  }
  return loops
}

/** The field's value at a floor point, bilinear. */
function valueAt(field, x, y) {
  const fx = (x - field.ox) / CELL
  const fy = (y - field.oy) / CELL
  const i = Math.max(0, Math.min(field.nx - 2, Math.floor(fx)))
  const j = Math.max(0, Math.min(field.ny - 2, Math.floor(fy)))
  const tx = fx - i
  const ty = fy - j
  const v = field.values
  const top = v[j * field.nx + i] * (1 - tx) + v[j * field.nx + i + 1] * tx
  const bottom = v[(j + 1) * field.nx + i] * (1 - tx) + v[(j + 1) * field.nx + i + 1] * tx
  return top * (1 - ty) + bottom * ty
}

/** Orients a loop so that the floor lies to the side of the normal (-dy, dx) of each segment. */
function oriented(field, loop) {
  const [a, b] = [loop[0], loop[1]]
  const dx = b[0] - a[0]
  const dy = b[1] - a[1]
  const len = sqrt(dx * dx + dy * dy) || 1
  const mx = (a[0] + b[0]) / 2 + (-dy / len) * CELL * 0.6
  const my = (a[1] + b[1]) / 2 + (dx / len) * CELL * 0.6
  return valueAt(field, mx, my) > 0 ? loop : loop.slice().reverse()
}

function simplifyChain(points, tolerance) {
  const keep = new Uint8Array(points.length)
  keep[0] = 1
  keep[points.length - 1] = 1
  const stack = [[0, points.length - 1]]
  while (stack.length) {
    const [lo, hi] = stack.pop()
    let worst = -1
    let at = -1
    for (let i = lo + 1; i < hi; i++) {
      const d = pointSegment(points[i][0], points[i][1], points[lo][0], points[lo][1], points[hi][0], points[hi][1])
      if (d > worst) { worst = d; at = i }
    }
    if (worst > tolerance) { keep[at] = 1; stack.push([lo, at], [at, hi]) }
  }
  return points.filter((_, i) => keep[i])
}

/** Douglas-Peucker on a closed loop: split at the point farthest from the first, simplify both halves. */
function simplifyLoop(loop, tolerance) {
  if (loop.length < 4) return loop
  let far = 0
  let best = -1
  for (let i = 1; i < loop.length; i++) {
    const dx = loop[i][0] - loop[0][0]
    const dy = loop[i][1] - loop[0][1]
    const d = dx * dx + dy * dy
    if (d > best) { best = d; far = i }
  }
  const first = simplifyChain(loop.slice(0, far + 1), tolerance)
  const second = simplifyChain(loop.slice(far).concat([loop[0]]), tolerance)
  return first.concat(second.slice(1, -1))
}

/* ---------------- Walls ---------------- */

function wallMesh(loops) {
  let quads = 0
  for (const loop of loops) quads += 2 * loop.length
  const positions = new Float32Array(quads * 4 * 3)
  const normals = new Float32Array(quads * 4 * 3)
  const shade = new Float32Array(quads * 4)
  const uvs = new Float32Array(quads * 4 * 2)
  const indices = new Uint32Array(quads * 6)
  let v = 0
  let q = 0
  // Three rings of heights; the lowest strip is darker, a cheap stand-in for ambient occlusion where wall meets floor.
  const rings = [[0, 0.35], [FOOT, 0.8], [WALL_HEIGHT, 1]]
  for (const loop of loops) {
    // `along` is the length of wall walked so far: the horizontal texture coordinate, so stone courses follow the wall round corners.
    let along = 0
    for (let k = 0; k < loop.length; k++) {
      const [ax, ay] = loop[k]
      const [bx, by] = loop[(k + 1) % loop.length]
      const dx = bx - ax
      const dy = by - ay
      const len = sqrt(dx * dx + dy * dy)
      if (len < 0.01) continue
      const nx = -dy / len
      const nz = dx / len
      for (let r = 0; r < 2; r++) {
        const [h0, s0] = rings[r]
        const [h1, s1] = rings[r + 1]
        const corners = [[ax, h0, ay, s0, along], [bx, h0, by, s0, along + len], [bx, h1, by, s1, along + len], [ax, h1, ay, s1, along]]
        for (const [x, y, z, s, u] of corners) {
          positions.set([x, y, z], v * 3)
          normals.set([nx, 0, nz], v * 3)
          uvs.set([u, y], v * 2)
          shade[v] = s
          v++
        }
        indices.set([v - 4, v - 3, v - 2, v - 4, v - 2, v - 1], q * 6)
        q++
      }
      along += len
    }
  }
  return { positions: positions.subarray(0, v * 3), normals: normals.subarray(0, v * 3), uvs: uvs.subarray(0, v * 2), shade: shade.subarray(0, v), indices: indices.subarray(0, q * 6), quads: q }
}

/* ---------------- Torches, pillars ---------------- */

function onRoomWall(floor, rooms, x, y) {
  for (const room of rooms) {
    if (Math.abs(shapeValue(room.shape, x, y)) < 3) return true
  }
  return false
}

function nearDoor(floor, x, y) {
  const reach = floor.params.width / 2 + 10
  for (const room of floor.rooms) {
    if (!room.alive) continue
    for (const port of room.used) {
      const dx = port.p[0] - x
      const dy = port.p[1] - y
      if (dx * dx + dy * dy < reach * reach) return true
    }
  }
  return false
}

/** Torch spots along the walls of rooms (not corridors), every `spacing` units of wall. [x, z, inward x, inward z] per torch. */
function torchesAt(floor, loops, spacing) {
  const rooms = floor.rooms.filter((r) => r.alive)
  const out = []
  for (const loop of loops) {
    let carried = spacing / 2
    for (let k = 0; k < loop.length; k++) {
      const [ax, ay] = loop[k]
      const [bx, by] = loop[(k + 1) % loop.length]
      const dx = bx - ax
      const dy = by - ay
      const len = sqrt(dx * dx + dy * dy)
      if (len < 0.01) continue
      // `next` is the distance from this segment's start to the next torch.
      let next = spacing - carried
      while (next <= len) {
        const x = ax + (dx / len) * next
        const y = ay + (dy / len) * next
        if (onRoomWall(floor, rooms, x, y) && !nearDoor(floor, x, y)) out.push(x + (-dy / len) * 6, y + (dx / len) * 6, -dy / len, dx / len)
        next += spacing
      }
      carried = spacing - (next - len)
    }
  }
  return out
}

function placeTorches(floor, loops) {
  let spacing = 170
  let torches = torchesAt(floor, loops, spacing)
  for (let n = 0; n < 12 && torches.length / 4 > 56; n++) torches = torchesAt(floor, loops, (spacing *= 1.15))
  for (let n = 0; n < 12 && torches.length / 4 < 24 && spacing > 60; n++) torches = torchesAt(floor, loops, (spacing *= 0.87))
  return Float32Array.from(torches)
}

function junctionPillars(floor) {
  const half = floor.params.width / 2
  const out = []
  for (const node of floor.nodes) {
    if (node.kind !== 'junction') continue
    const [x, y] = node.p
    out.push(x - half, y - half, x + half, y - half, x - half, y + half, x + half, y + half)
  }
  return Float32Array.from(out)
}

/* ---------------- Things lying about: bones, skulls, webs ---------------- */

/** A small deterministic generator, so the same floor always gets the same bones. */
function rng(seed) {
  let a = (seed * 2654435761 + 1013904223) >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

function distToPath(points, x, y) {
  let best = Infinity
  for (let i = 1; i < points.length; i++) best = Math.min(best, pointSegment(x, y, points[i - 1][0], points[i - 1][1], points[i][0], points[i][1]))
  return best
}

/** Bones and skulls inside rooms, kept off the pencilled route and away from the walls. [x, z, angle, scale] per piece. */
function propsAt(floor, points) {
  const bones = []
  const skulls = []
  for (const room of floor.rooms) {
    if (!room.alive) continue
    const rand = rng(room.id + 7)
    const s = room.shape
    const [x0, y0, x1, y1] =
      s.kind === 'disc' ? [s.c[0] - s.r, s.c[1] - s.r, s.c[0] + s.r, s.c[1] + s.r] : s.pts.reduce((b, [x, y]) => [Math.min(b[0], x), Math.min(b[1], y), Math.max(b[2], x), Math.max(b[3], y)], [Infinity, Infinity, -Infinity, -Infinity])
    const area = Math.min(((x1 - x0) * (y1 - y0)) / 9000, 6)
    const piles = 1 + Math.floor(area * 0.8 + rand() * 2)
    for (let n = 0, placed = 0; n < 40 && placed < piles; n++) {
      const x = x0 + rand() * (x1 - x0)
      const y = y0 + rand() * (y1 - y0)
      if (shapeValue(s, x, y) < 22 || distToPath(points, x, y) < 30) continue
      placed++
      const kinds = 2 + Math.floor(rand() * 3)
      for (let k = 0; k < kinds; k++) {
        const px = x + (rand() - 0.5) * 22
        const py = y + (rand() - 0.5) * 22
        if (shapeValue(s, px, py) < 12 || distToPath(points, px, py) < 20) continue
        bones.push(px, py, rand() * Math.PI * 2, 0.8 + rand() * 0.7)
      }
      if (rand() < 0.4) skulls.push(x + (rand() - 0.5) * 10, y + (rand() - 0.5) * 10, rand() * Math.PI * 2, 0.85 + rand() * 0.4)
    }
  }
  return { bones: Float32Array.from(bones), skulls: Float32Array.from(skulls) }
}

/** Cobwebs in the upper corners of rooms: [x, z, dx1, dz1, dx2, dz2, size], the corner and the unit vectors along its two walls. */
function websAt(floor, loops) {
  const rooms = floor.rooms.filter((r) => r.alive)
  const out = []
  for (const loop of loops) {
    for (let k = 0; k < loop.length; k++) {
      const [ax, ay] = loop[(k + loop.length - 1) % loop.length]
      const [bx, by] = loop[k]
      const [cx, cy] = loop[(k + 1) % loop.length]
      const l1 = sqrt((bx - ax) ** 2 + (by - ay) ** 2)
      const l2 = sqrt((cx - bx) ** 2 + (cy - by) ** 2)
      if (l1 < 14 || l2 < 14) continue
      const d1x = (bx - ax) / l1
      const d1y = (by - ay) / l1
      const d2x = (cx - bx) / l2
      const d2y = (cy - by) / l2
      // The floor lies to the left of travel, so a left turn is a corner of a room, a right turn a pier jutting into it.
      if (d1x * d2y - d1y * d2x < 0.5 || !onRoomWall(floor, rooms, bx, by) || nearDoor(floor, bx, by)) continue
      out.push(bx, by, -d1x, -d1y, d2x, d2y, 16 + ((k * 37) % 9))
    }
  }
  return Float32Array.from(out)
}

/** The route as a ribbon on the floor, `width` wide: positions and [distance along, across] per vertex, so the dashes stay sharp at any size. */
function routeRibbon(points, lengths, width) {
  const n = points.length
  const positions = new Float32Array(n * 2 * 3)
  const uvs = new Float32Array(n * 2 * 2)
  const indices = []
  for (let i = 0; i < n; i++) {
    const [px, py] = points[Math.max(0, i - 1)]
    const [nx, ny] = points[Math.min(n - 1, i + 1)]
    const dx = nx - px
    const dy = ny - py
    const len = sqrt(dx * dx + dy * dy) || 1
    // Averaged normal at a bend; its length is capped so a sharp corner does not spike.
    const [ax, ay] = points[i]
    const bx = ax - px
    const by = ay - py
    const cx = nx - ax
    const cy = ny - ay
    const lb = sqrt(bx * bx + by * by) || 1
    const lc = sqrt(cx * cx + cy * cy) || 1
    let mx = -(by / lb + cy / lc)
    let my = bx / lb + cx / lc
    const ml = sqrt(mx * mx + my * my)
    if (ml < 0.4) { mx = -dy / len; my = dx / len } else { mx /= ml; my /= ml }
    const [tx, ty] = [-dy / len, dx / len]
    const scale = 1 / Math.max(0.5, mx * tx + my * ty)
    for (const side of [-1, 1]) {
      const v = i * 2 + (side < 0 ? 0 : 1)
      positions.set([ax + mx * scale * width * side * 0.5, 0.2, ay + my * scale * width * side * 0.5], v * 3)
      uvs.set([lengths[i], side], v * 2)
    }
    if (i) indices.push(i * 2 - 2, i * 2 - 1, i * 2, i * 2 - 1, i * 2 + 1, i * 2)
  }
  return { positions, uvs, indices: Uint32Array.from(indices) }
}

/* ---------------- The path and the stations ---------------- */

function pathPoints(floor) {
  const nodeAt = (id) => {
    const node = floor.nodes[id]
    return node.kind === 'junction' ? node.p : floor.rooms[node.room].center
  }
  const points = []
  const mark = []
  const push = (p) => {
    const last = points[points.length - 1]
    if (last && Math.abs(last[0] - p[0]) < 1 && Math.abs(last[1] - p[1]) < 1) return points.length - 1
    points.push(p)
    return points.length - 1
  }
  floor.mainPath.forEach((id, k) => {
    if (k > 0) {
      const from = floor.mainPath[k - 1]
      const edge = floor.edges.find((e) => (e.u === from && e.v === id) || (e.u === id && e.v === from))
      if (edge) for (const p of edge.u === from ? edge.pts : edge.pts.slice().reverse()) push(p)
    }
    const at = push(nodeAt(id))
    mark.push({ node: id, at })
  })
  return { points, mark }
}

/** One doorway per end of every corridor, on the wall of its room: [x, z, dx, dz] with (dx, dz) the unit vector pointing down the corridor. */
function doorsAt(floor) {
  const out = []
  for (const edge of floor.edges) {
    const n = edge.pts.length
    if (n < 2) continue
    for (const [at, from] of [[0, 1], [n - 1, n - 2]]) {
      const [x, y] = edge.pts[at]
      const [ox, oy] = edge.pts[from]
      const len = sqrt((ox - x) ** 2 + (oy - y) ** 2)
      if (len < 0.01) continue
      out.push(x, y, (ox - x) / len, (oy - y) / len)
    }
  }
  return Float32Array.from(out)
}

/** Rounds every bend of the walk: each corner is replaced by a quadratic curve that starts and ends `cut` units from it (less on short
 *  stretches), so the head turns through the bend instead of snapping at it. `cutAt(i)` is the cut of vertex i: small in corridors (46 stays
 *  inside a 44-wide one), large in a room, where the walk cuts across towards the way out instead of going to the middle first. */
function smoothPath(points, cutAt) {
  const out = [points[0]]
  for (let i = 1; i < points.length - 1; i++) {
    const [ax, ay] = points[i - 1]
    const [bx, by] = points[i]
    const [cx, cy] = points[i + 1]
    const l1 = sqrt((ax - bx) ** 2 + (ay - by) ** 2)
    const l2 = sqrt((cx - bx) ** 2 + (cy - by) ** 2)
    const d = Math.min(cutAt(i), l1 / 2, l2 / 2)
    const turn = ((bx - ax) * (cx - bx) + (by - ay) * (cy - by)) / (l1 * l2 || 1)
    if (d < 2 || turn > 0.995) { out.push(points[i]); continue }
    const p0 = [bx + ((ax - bx) / l1) * d, by + ((ay - by) / l1) * d]
    const p2 = [bx + ((cx - bx) / l2) * d, by + ((cy - by) / l2) * d]
    const steps = Math.max(4, Math.ceil(d / 4))
    for (let k = 0; k <= steps; k++) {
      const t = k / steps
      const u = 1 - t
      out.push([u * u * p0[0] + 2 * u * t * bx + t * t * p2[0], u * u * p0[1] + 2 * u * t * by + t * t * p2[1]])
    }
  }
  out.push(points[points.length - 1])
  return out
}

/** Inside a room the walk does not go to the middle and turn there: between the door it comes in by and the door it leaves by it follows one
 *  cubic curve that leaves along the corridor it came from and arrives along the one it takes next. `rooms` holds the indices of the room
 *  vertices to replace; the first and last room keep theirs (the walk starts and ends there). */
function cutRooms(points, rooms) {
  const out = []
  const unit = (ax, ay, bx, by) => {
    const l = sqrt((bx - ax) ** 2 + (by - ay) ** 2)
    return l < 1 ? null : [(bx - ax) / l, (by - ay) / l]
  }
  for (let i = 0; i < points.length; i++) {
    const a = points[i - 1]
    const b = points[i + 1]
    if (!rooms.has(i) || !a || !b || !points[i - 2] || !points[i + 2]) { out.push(points[i]); continue }
    const tin = unit(...points[i - 2], ...a) ?? unit(...a, ...points[i])
    const tout = unit(...b, ...points[i + 2]) ?? unit(...points[i], ...b)
    const reach = 0.5 * sqrt((b[0] - a[0]) ** 2 + (b[1] - a[1]) ** 2)
    const c1 = [a[0] + tin[0] * reach, a[1] + tin[1] * reach]
    const c2 = [b[0] - tout[0] * reach, b[1] - tout[1] * reach]
    for (let k = 1; k < 8; k++) {
      const t = k / 8
      const u = 1 - t
      out.push([u * u * u * a[0] + 3 * u * u * t * c1[0] + 3 * u * t * t * c2[0] + t * t * t * b[0], u * u * u * a[1] + 3 * u * u * t * c1[1] + 3 * u * t * t * c2[1] + t * t * t * b[1]])
    }
  }
  return out
}

/** Arc length along `points` (with cumulative `lengths`) of the point nearest to (x, y). */
function arcAt(points, lengths, x, y) {
  let best = Infinity
  let at = 0
  for (let i = 1; i < points.length; i++) {
    const [ax, ay] = points[i - 1]
    const [bx, by] = points[i]
    const dx = bx - ax
    const dy = by - ay
    const len2 = dx * dx + dy * dy
    const t = len2 ? Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / len2)) : 0
    const d = (ax + t * dx - x) ** 2 + (ay + t * dy - y) ** 2
    if (d < best) { best = d; at = lengths[i - 1] + t * (lengths[i] - lengths[i - 1]) }
  }
  return at
}

/** Where the walk starts: inside the first room, as far back from its way out as the room allows, so the camera looks at the way in. */
function introPoint(field, points) {
  const [cx, cy] = points[0]
  const [nx, ny] = points.find((p) => Math.abs(p[0] - cx) > 1 || Math.abs(p[1] - cy) > 1) ?? [cx + 1, cy]
  const len = sqrt((nx - cx) ** 2 + (ny - cy) ** 2) || 1
  const dx = (cx - nx) / len
  const dy = (cy - ny) / len
  let back = 0
  while (back < 400 && valueAt(field, cx + dx * (back + CELL), cy + dy * (back + CELL)) > 12) back += CELL
  return [cx + dx * back, cy + dy * back]
}

/* ---------------- Everything ---------------- */

/** `route` is an optional hand-made walk (see walk/route.json and walk/editor.js): `waypoints` [[x, z], ...] replace the generated path (the
 *  first is where the walk starts) and `stations` [{x, z, rest}] say where the walk stops in each of the seven rooms and for how long
 *  (`rest`, in screen heights). Anything left out is generated. */
export function build(floor, route = null) {
  const field = sampleField(floor)
  const loops = contours(field)
    .map((loop) => simplifyLoop(oriented(field, loop), SIMPLIFY))
    .filter((loop) => loop.length >= 3)
    .sort((a, b) => {
      const ka = a.reduce((m, p) => [Math.min(m[0], p[0]), Math.min(m[1], p[1])], [Infinity, Infinity])
      const kb = b.reduce((m, p) => [Math.min(m[0], p[0]), Math.min(m[1], p[1])], [Infinity, Infinity])
      return ka[0] - kb[0] || ka[1] - kb[1]
    })

  const walked = pathPoints(floor)
  const intro = introPoint(field, walked.points)
  const roomAt = walked.mark.filter((m) => floor.nodes[m.node].kind === 'room').map((m) => m.at + 1)
  const raw = [intro, ...walked.points]
  const edited = route?.waypoints?.length >= 2
  const points = edited ? smoothPath(route.waypoints.map(([x, z]) => [x, z]), () => 46) : smoothPath(cutRooms(raw, new Set(roomAt.slice(1, -1))), () => 46)
  const lengths = new Float32Array(points.length)
  for (let i = 1; i < points.length; i++) {
    const dx = points[i][0] - points[i - 1][0]
    const dy = points[i][1] - points[i - 1][1]
    lengths[i] = lengths[i - 1] + sqrt(dx * dx + dy * dy)
  }
  const total = lengths[lengths.length - 1] || 1
  const roomMarks = walked.mark.filter((m) => floor.nodes[m.node].kind === 'room')
  const stations = Array.from({ length: STATIONS }, (_, i) => {
    const m = roomMarks[Math.round((i * (roomMarks.length - 1)) / (STATIONS - 1))]
    const room = floor.nodes[m.node].room
    const [x, y] = floor.rooms[room].center
    // The walk begins at the intro point, a little before the first room's centre: station 0 is that start. The others are where the
    // rounded path passes closest to their room's centre.
    const mine = route?.stations?.[i]
    const [sx, sy] = mine && edited ? [mine.x, mine.z] : [x, y]
    return { room, x: sx, y: sy, u: i === 0 ? 0 : arcAt(points, lengths, sx, sy) / total, rest: mine?.rest ?? 1.3 }
  })

  const leaf = floor.rooms.find((r) => r.alive && r.leaf)
  const stairs = floor.rooms[floor.exit]
  const mask = new Uint8Array(field.nx * field.ny)
  for (let i = 0; i < mask.length; i++) mask[i] = field.values[i] > 0 ? 255 : 0
  const stairsShape = stairs.shape
  const stairsRadius =
    stairsShape.kind === 'disc'
      ? stairsShape.r
      : stairsShape.pts.reduce((m, [x, y]) => Math.min(m, pointSegment(stairs.center[0], stairs.center[1], x, y, x, y)), Infinity)

  return {
    bounds: field.bounds,
    mask: { data: mask, nx: field.nx, ny: field.ny, ox: field.ox, oy: field.oy, cell: CELL },
    walls: wallMesh(loops),
    props: propsAt(floor, points),
    webs: websAt(floor, loops),
    route: routeRibbon(points, lengths, 1.6),
    torches: placeTorches(floor, loops),
    pillars: junctionPillars(floor),
    doors: doorsAt(floor),
    intro,
    path: { points: Float32Array.from(points.flatMap(([x, y]) => [x, 0, y])), lengths, total },
    stations,
    waypoints: simplifyChain(points, 3),
    leafRoom: leaf ? { id: leaf.id, center: leaf.center, cls: leaf.cls } : null,
    stairs: { room: floor.exit, center: stairs.center, radius: stairsRadius },
    contours: loops.length,
  }
}
