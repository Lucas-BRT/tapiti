// The dungeon: a fixed three.js scene behind the landing page, seen in the first person. Scrolling walks the camera, at eye height, along the
// main path of one real floor (the one the generator draws for `data-seed` and `data-size`, built here by the same wasm module the lab uses).
// The rooms are the generator's own; paper, pencil hatching and a pink route give the floor its Fieldnotes drawing style.
// Scrolling stays native: this file only reads the scroll position. Without WebGL2, scripts or consent to data, the page keeps the
// server-rendered SVG of the same floor behind it. Geometry comes from build.js (pure data); this file owns everything that touches the GPU.

const root = document.querySelector('.walk-backdrop[data-three]')

const params = new URLSearchParams(location.search)
const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches || params.get('motion') === 'reduce'
const saveData = () => navigator.connection?.saveData === true

/** Colours come from the page's own tokens, so CSS stays the one place that says what the stone and the torches look like. */
const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim()
const clamp = (x, lo, hi) => (x < lo ? lo : x > hi ? hi : x)
const smooth = (a, b, x) => { const t = clamp((x - a) / (b - a), 0, 1); return t * t * (3 - 2 * t) }
const lerp = (a, b, t) => a + (b - a) * t

/** Human scale: a corridor is 44 units wide, about 2.5 m, so a metre is 17.6 units. */
const EYE = 29
const WALL = 60
const LOOK = 90

function hasWebGL2() {
  try {
    return !!document.createElement('canvas').getContext('webgl2')
  } catch {
    return false
  }
}

function remember(key, value, store = localStorage) {
  try {
    if (value === undefined) return store.getItem(key)
    store.setItem(key, value)
  } catch {
    return null
  }
}

/** Paper flags and the pencilled route, clipped by the floor mask. */
function flagTexture(THREE, scene, size) {
  const [x0, y0, x1, y1] = scene.bounds
  const W = size
  const H = Math.round((size * (y1 - y0)) / (x1 - x0))
  const sx = W / (x1 - x0)
  const sy = H / (y1 - y0)
  const canvas = document.createElement('canvas')
  canvas.width = W
  canvas.height = H
  const c = canvas.getContext('2d')
  const m = scene.mask
  const small = document.createElement('canvas')
  small.width = m.nx
  small.height = m.ny
  const image = small.getContext('2d').createImageData(m.nx, m.ny)
  for (let i = 0; i < m.data.length; i++) { image.data[i * 4] = image.data[i * 4 + 1] = image.data[i * 4 + 2] = 255; image.data[i * 4 + 3] = m.data[i] }
  small.getContext('2d').putImageData(image, 0, 0)
  c.imageSmoothingEnabled = true
  c.drawImage(small, (m.ox - x0 + m.cell / 2) * sx, (m.oy - y0 + m.cell / 2) * sy, m.nx * m.cell * sx, m.ny * m.cell * sy)
  c.globalCompositeOperation = 'source-in'
  c.fillStyle = css('--scene-floor')
  c.fillRect(0, 0, W, H)
  c.globalCompositeOperation = 'source-atop'
  const flagW = 28 * sx
  const flagH = 28 * sy
  c.globalAlpha = 0.2
  c.strokeStyle = css('--scene-ink')
  c.lineWidth = 0.8
  for (let row = 0, y = 0; y < H; row++, y += flagH) {
    c.beginPath()
    c.moveTo(0, y)
    c.lineTo(W, y)
    for (let x = -((row * 0.5 + 0.3) % 1) * flagW; x < W; x += flagW) { c.moveTo(x, y); c.lineTo(x, y + flagH) }
    c.stroke()
  }
  c.globalAlpha = 1
  c.strokeStyle = css('--scene-route')
  c.lineWidth = 1.8 * sx
  c.setLineDash([8 * sx, 6 * sx])
  c.beginPath()
  const points = scene.path.points
  for (let i = 0; i < points.length; i += 3) {
    const x = (points[i] - x0) * sx, y = (points[i + 2] - y0) * sy
    if (i) c.lineTo(x, y)
    else c.moveTo(x, y)
  }
  c.stroke()
  c.setLineDash([])
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  texture.anisotropy = 8
  return texture
}

/* ---------------- Mount ---------------- */

/** Builds the scene for `floor` on `canvas` and returns a handle. `THREE` is the vendored module, `scene` the result of build.js. */
export function mount({ THREE, paper, canvas, floor, scene: built, page }) {
  const abyss = new THREE.Color(css('--abyss'))
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: devicePixelRatio < 1.5, powerPreference: 'low-power' })
  renderer.outputColorSpace = THREE.SRGBColorSpace
  renderer.setClearColor(abyss, 1)

  const scene = new THREE.Scene()
  scene.background = abyss
  scene.fog = new THREE.FogExp2(abyss, 0.0024)
  const camera = new THREE.PerspectiveCamera(70, 1, 2, 2500)

  /* Unlit graphite and paper: shading comes from the hatch, so it stays stable while scrolling. */
  const wallGeometry = new THREE.BufferGeometry()
  const w = built.walls
  wallGeometry.setAttribute('position', new THREE.BufferAttribute(w.positions, 3))
  wallGeometry.setAttribute('normal', new THREE.BufferAttribute(w.normals, 3))
  wallGeometry.setAttribute('uv', new THREE.BufferAttribute(w.uvs, 2))
  wallGeometry.setIndex(new THREE.BufferAttribute(w.indices, 1))
  const wallMaterial = paper.wallMaterial(THREE, css('--scene-wall'), css('--scene-ink'))
  scene.add(new THREE.Mesh(wallGeometry, wallMaterial))

  /* The paper floor keeps the real outline; the untouched page is the sky. */
  const [bx0, by0, bx1, by1] = built.bounds
  const texture = flagTexture(THREE, built, 2048)
  const plane = new THREE.PlaneGeometry(bx1 - bx0, by1 - by0)
  const floorMaterial = new THREE.MeshBasicMaterial({ map: texture, alphaTest: 0.5 })
  // Keep the stairwell open in the floor plane.
  const wellSide = Math.min(built.stairs.radius * 1.1, 110)
  const well = [built.stairs.center[0] - wellSide / 2, built.stairs.center[1] - wellSide / 2, built.stairs.center[0] + wellSide / 2, built.stairs.center[1] + wellSide / 2]
  floorMaterial.onBeforeCompile = (shader) => {
    shader.uniforms.uWell = { value: Float32Array.from(well) }
    shader.vertexShader = shader.vertexShader.replace('void main() {', 'varying vec3 vFloorPos;\nvoid main() {').replace('#include <begin_vertex>', '#include <begin_vertex>\nvFloorPos = (modelMatrix * vec4(position, 1.0)).xyz;')
    shader.fragmentShader = shader.fragmentShader.replace('void main() {', 'uniform vec4 uWell;\nvarying vec3 vFloorPos;\nvoid main() {\n  if (vFloorPos.x > uWell.x && vFloorPos.x < uWell.z && vFloorPos.z > uWell.y && vFloorPos.z < uWell.w) discard;')
  }
  const ground = new THREE.Mesh(plane, floorMaterial)
  ground.rotation.x = -Math.PI / 2
  ground.position.set((bx0 + bx1) / 2, 0, (by0 + by1) / 2)
  scene.add(ground)
  /* Corner pillars at the junctions hide the seams where corridors cross. */
  const pillars = built.pillars.length / 2
  const pillarMaterial = paper.wallMaterial(THREE, css('--scene-wall'), css('--scene-ink'))
  const pillarGeometry = new THREE.BoxGeometry(7, WALL, 7)
  // Box faces carry uv in [0, 1]; scale them to wall units so the courses match the walls.
  const boxUv = pillarGeometry.attributes.uv
  for (let i = 0; i < boxUv.count; i++) boxUv.setXY(i, boxUv.getX(i) * 7, boxUv.getY(i) * WALL)
  let pillarMesh = null
  if (pillars) {
    pillarMesh = new THREE.InstancedMesh(pillarGeometry, pillarMaterial, pillars)
    const m = new THREE.Matrix4()
    for (let i = 0; i < pillars; i++) pillarMesh.setMatrixAt(i, m.makeTranslation(built.pillars[i * 2], WALL / 2, built.pillars[i * 2 + 1]))
    scene.add(pillarMesh)
  }

  /* Doorways: two jambs and a lintel where each corridor meets its room, so a room ends and a passage begins. */
  const doors = built.doors.length / 4
  const jambGeometry = new THREE.BoxGeometry(6, 46, 6)
  const lintelGeometry = new THREE.BoxGeometry(56, 14, 7)
  for (const g of [jambGeometry, lintelGeometry]) {
    const uv = g.attributes.uv
    const [gw, gh] = g === jambGeometry ? [6, 46] : [56, 14]
    for (let i = 0; i < uv.count; i++) uv.setXY(i, uv.getX(i) * gw, uv.getY(i) * gh)
  }
  const doorMaterial = paper.wallMaterial(THREE, css('--scene-door'), css('--scene-ink'))
  const jambs = new THREE.InstancedMesh(jambGeometry, doorMaterial, doors * 2)
  const lintels = new THREE.InstancedMesh(lintelGeometry, doorMaterial, doors)
  const dm = new THREE.Matrix4()
  const dq = new THREE.Quaternion()
  const dpos = new THREE.Vector3()
  const dscale = new THREE.Vector3(1, 1, 1)
  const up = new THREE.Vector3(0, 1, 0)
  for (let i = 0; i < doors; i++) {
    const [x, z, dx, dz] = built.doors.subarray(i * 4, i * 4 + 4)
    // The box's x axis lies across the corridor, its z along it.
    dq.setFromAxisAngle(up, Math.atan2(dx, dz))
    const side = [-dz, dx]
    for (const sgn of [-1, 1]) {
      dpos.set(x + side[0] * sgn * 19, 23, z + side[1] * sgn * 19)
      jambs.setMatrixAt(i * 2 + (sgn < 0 ? 0 : 1), dm.compose(dpos, dq, dscale))
    }
    dpos.set(x, 46 + 7, z)
    lintels.setMatrixAt(i, dm.compose(dpos, dq, dscale))
  }
  scene.add(jambs, lintels)

  /* Darker pencil steps below the paper floor. */
  const stairs = built.stairs
  const side = Math.min(stairs.radius * 1.1, 110)
  const stepMaterial = paper.wallMaterial(THREE, css('--scene-step'), css('--scene-ink'))
  const stepGeometry = new THREE.BoxGeometry(side, 20, side / 8)
  const stepUv = stepGeometry.attributes.uv
  for (let i = 0; i < stepUv.count; i++) stepUv.setXY(i, stepUv.getX(i) * side, stepUv.getY(i) * 20)
  for (let k = 0; k < 8; k++) {
    const step = new THREE.Mesh(stepGeometry, stepMaterial)
    // The first step's top sits 6 units under the floor, never in its plane (that showed as a bar across the flags).
    step.position.set(stairs.center[0], -16 - k * 20, stairs.center[1] - side / 2 + (k + 0.5) * (side / 8))
    scene.add(step)
  }
  /* Drawn torches stay still on the paper; they need no point lights. */
  const count = built.torches.length / 4
  const bracketGeometry = new THREE.BoxGeometry(3, 12, 3)
  const bracketMaterial = new THREE.MeshBasicMaterial({ color: css('--scene-ink') })
  const brackets = new THREE.InstancedMesh(bracketGeometry, bracketMaterial, count)
  const flameGeometry = new THREE.PlaneGeometry(1, 1)
  const flames = new THREE.InstancedMesh(flameGeometry, null, count)
  const lit = new Float32Array(count).fill(1)
  const m4 = new THREE.Matrix4()
  for (let i = 0; i < count; i++) {
    const x = built.torches[i * 4]
    const z = built.torches[i * 4 + 1]
    brackets.setMatrixAt(i, m4.makeTranslation(x, 31, z))
    flames.setMatrixAt(i, m4.makeTranslation(x, 38, z))
  }
  const litAttribute = new THREE.InstancedBufferAttribute(lit, 1)
  flames.geometry.setAttribute('lit', litAttribute)
  const flameUniforms = { uColor: { value: new THREE.Color(css('--light-torch')) }, uCore: { value: new THREE.Color('#f1d58c') }, uInk: { value: new THREE.Color(css('--scene-ink')) } }
  flames.material = new THREE.ShaderMaterial({ vertexShader: paper.FLAME_VERTEX, fragmentShader: paper.FLAME_FRAGMENT, uniforms: flameUniforms, transparent: true, depthWrite: false })
  flames.frustumCulled = false
  scene.add(brackets, flames)
  /* The path: where the eyes are along it, and which way they look. */
  const pts = built.path.points
  const lens = built.path.lengths
  const total = built.path.total
  const pointAt = (u, out) => {
    const d = clamp(u, 0, 1) * total
    let lo = 0
    let hi = lens.length - 1
    while (hi - lo > 1) { const mid = (lo + hi) >> 1; if (lens[mid] <= d) lo = mid; else hi = mid }
    const span = lens[hi] - lens[lo] || 1
    const t = clamp((d - lens[lo]) / span, 0, 1)
    out[0] = lerp(pts[lo * 3], pts[hi * 3], t)
    out[2] = lerp(pts[lo * 3 + 2], pts[hi * 3 + 2], t)
    return out
  }
  const stationU = built.stations.map((s) => s.u)
  const here = [0, 0, 0]
  const ahead = [0, 0, 0]
  const pose = { px: 0, py: EYE, pz: 0, yaw: 0, pitch: 0 }
  let yaw = null
  const headingAt = (u) => {
    pointAt(u, here)
    // Look at a point a little way along the path; where it turns, the head follows a moment later (see `yaw` in render).
    pointAt(u + LOOK / total, ahead)
    // At the very end of the path there is no "ahead": keep the heading of the last stretch.
    if (Math.hypot(ahead[0] - here[0], ahead[2] - here[2]) < 1) {
      pointAt(u - LOOK / total, ahead)
      return Math.atan2(here[0] - ahead[0], here[2] - ahead[2])
    }
    return Math.atan2(ahead[0] - here[0], ahead[2] - here[2])
  }

  /** The camera pose for walk position `u` and footer descent `dive` (0 to 1). */
  const poseAt = (u, dive, bob) => {
    const heading = headingAt(u)
    pose.px = here[0]
    pose.pz = here[2]
    pose.py = EYE + bob
    pose.yaw = heading
    pose.pitch = -0.04
    if (dive > 0) {
      // Down the stairwell and into the dark: the head tips toward the steps.
      pose.px = lerp(pose.px, stairs.center[0], dive)
      pose.pz = lerp(pose.pz, stairs.center[1] + 30 * (1 - dive), dive)
      pose.py = lerp(pose.py, -90, dive)
      pose.pitch = lerp(pose.pitch, -0.7, dive)
    }
    return pose
  }

  /* Text in the world: every room's text is a sign standing in that room, ahead of where the walk stops there. The signs are the page's own
     elements (links, field and button keep working, and so does reading and focus), placed with the camera's matrices so they sit in the
     picture's perspective. The canvas knows nothing of them, so a sign fades out as the walker leaves its room instead of showing through walls. */
  const DEPTH = 130
  const bodies = page ? [...page.querySelectorAll('[data-station] .room-body')] : []
  const layer = document.createElement('div')
  layer.className = 'world-text'
  const rig = document.createElement('div')
  rig.className = 'world-rig'
  layer.append(rig)
  const signs = bodies.map((el) => {
    const station = Number(el.closest('[data-station]').dataset.station)
    const at = stationU[station]
    const heading = headingAt(at)
    const sign = { el, station, base: [here[0] + Math.sin(heading) * DEPTH, here[2] + Math.cos(heading) * DEPTH], yaw: heading + Math.PI, left: [Math.cos(heading), -Math.sin(heading)] }
    return sign
  })
  const view = new THREE.Matrix4()
  const object = new THREE.Matrix4()
  const quat = new THREE.Quaternion()
  const position = new THREE.Vector3()
  const scale = new THREE.Vector3()
  const upAxis = new THREE.Vector3(0, 1, 0)
  const e = (n) => (Math.abs(n) < 1e-10 ? 0 : n)
  const placeSigns = () => {
    const W = canvas.clientWidth
    const H = canvas.clientHeight
    const fov = (0.5 / Math.tan((camera.fov * Math.PI) / 360)) * H
    layer.style.perspective = `${fov}px`
    rig.style.width = `${W}px`
    rig.style.height = `${H}px`
    camera.updateMatrixWorld()
    view.copy(camera.matrixWorld).invert()
    const c = view.elements
    rig.style.transform = `translateZ(${fov}px) matrix3d(${e(c[0])},${e(-c[1])},${e(c[2])},${e(c[3])},${e(c[4])},${e(-c[5])},${e(c[6])},${e(c[7])},${e(c[8])},${e(-c[9])},${e(c[10])},${e(c[11])},${e(c[12])},${e(-c[13])},${e(c[14])},${e(c[15])}) translate(${W / 2}px,${H / 2}px)`
    const k = DEPTH / fov
    for (const sign of signs) {
      // On wide screens a sign stands to the left of the way ahead, so the way stays in sight; on narrow ones it is centred.
      const panel = sign.el.offsetWidth
      const shift = Math.max(0, W / 2 - panel / 2 - 56) * k
      position.set(sign.base[0] + sign.left[0] * shift, EYE, sign.base[1] + sign.left[1] * shift)
      // A sign shows only while the walk is near its room (by the length of path walked, not by how far it looks): it is gone before the
      // walker is past it, so it is never seen huge and half-faded from the side, and two signs are never up at once.
      const eye = position.clone().applyMatrix4(view)
      const opacity = 1 - smooth(10, 55, Math.abs(state.u - stationU[sign.station]) * total)
      sign.el.style.opacity = opacity.toFixed(3)
      sign.el.style.visibility = opacity < 0.02 ? 'hidden' : 'visible'
      sign.el.style.pointerEvents = opacity > 0.6 ? 'auto' : 'none'
      sign.el.inert = opacity <= 0.6
      // The sign turns most of the way to face the walker, so it never reads at a steep angle while the walk goes round a bend.
      let turn = yaw + look.yaw + Math.PI - sign.yaw
      turn -= Math.round(turn / (2 * Math.PI)) * 2 * Math.PI
      quat.setFromAxisAngle(upAxis, sign.yaw + turn * 0.75)
      // And it stays on the screen: past its edges the sign slides along the view, so it is never cut by the window.
      const depth = -eye.z
      if (opacity > 0.02 && depth > 20) {
        const sx = W / 2 + (eye.x * fov) / depth
        const sy = H / 2 - (eye.y * fov) / depth
        const halfW = (panel * DEPTH) / (2 * depth)
        const halfH = (sign.el.offsetHeight * DEPTH) / (2 * depth)
        const fit = (v, lo, hi, mid) => (hi - lo < 0 ? mid : clamp(v, lo, hi))
        const nx = fit(sx, 20 + halfW, W - 20 - halfW, W / 2)
        const ny = fit(sy, 72 + halfH, H - 48 - halfH, H / 2)
        const m = camera.matrixWorld.elements
        const dx = ((nx - sx) * depth) / fov
        const dy = (-(ny - sy) * depth) / fov
        position.x += m[0] * dx + m[4] * dy
        position.y += m[1] * dx + m[5] * dy
        position.z += m[2] * dx + m[6] * dy
      }
      scale.set(k, k, k)
      object.compose(position, quat, scale)
      const o = object.elements
      sign.el.style.transform = `translate(-50%,-50%) matrix3d(${e(o[0])},${e(o[1])},${e(o[2])},${e(o[3])},${e(-o[4])},${e(-o[5])},${e(-o[6])},${e(-o[7])},${e(o[8])},${e(o[9])},${e(o[10])},${e(o[11])},${e(o[12])},${e(o[13])},${e(o[14])},${e(o[15])})`
    }
  }
  const mountSigns = () => {
    if (!signs.length || layer.isConnected) return
    page.before(layer)
    for (const sign of signs) rig.append(sign.el)
  }

  /* Looking around: the pointer turns the head a little, so the sign and the stone move against each other. */
  const look = { x: 0, y: 0, yaw: 0, pitch: 0 }
  addEventListener('pointermove', (event) => {
    if (reduced()) return
    look.x = event.clientX / innerWidth - 0.5
    look.y = event.clientY / innerHeight - 0.5
    wake()
  }, { passive: true })

  /* Scroll to walk position */
  const sections = page ? [...page.querySelectorAll('[data-station]')] : []
  const footer = document.querySelector('.site-footer')
  let tops = []
  let heights = []
  // Scrolling is walking: every metre of corridor costs the same amount of scroll, so the walk has one speed whatever the room. A room's
  // section is a rest (the walk stands in the room, its sign in view) and then the stretch to the next room, as long as that stretch is.
  const PX_PER_UNIT = 1.4
  const restPx = () => innerHeight * 0.55
  const measure = () => {
    sections.forEach((el, i) => {
      const last = i === sections.length - 1
      const stretch = last ? 0 : (stationU[i + 1] - stationU[i]) * total * PX_PER_UNIT
      el.style.height = `${Math.round(last ? innerHeight : restPx() + stretch)}px`
    })
    const base = scrollY
    tops = sections.map((el) => el.getBoundingClientRect().top + base)
    heights = sections.map((el) => el.getBoundingClientRect().height)
  }
  const target = () => {
    if (!sections.length) return { u: 0, dive: 0, station: 0 }
    const vh = innerHeight
    const y = scrollY + 1
    let i = 0
    for (let k = 0; k < sections.length; k++) if (tops[k] <= y) i = k
    const local = y - tops[i]
    const rest = restPx()
    let u = stationU[i]
    let station = i
    if (i < stationU.length - 1 && local > rest) {
      const f = clamp((local - rest) / Math.max(1, heights[i] - rest), 0, 1)
      u = lerp(stationU[i], stationU[i + 1], f)
      station = f > 0.5 ? i + 1 : i
    }
    let dive = 0
    if (footer) {
      const rect = footer.getBoundingClientRect()
      dive = clamp((vh - rect.top) / Math.max(1, Math.min(rect.height, vh * 0.6)), 0, 1)
    }
    return { u, dive, station }
  }

  /* State */
  let state = { u: 0, dive: 0, station: -1 }
  let want = { u: 0, dive: 0, station: 0 }
  let stepped = reduced()
  const paused = () => remember('tapiti.walk.paused') === '1'
  if (paused()) stepped = true
  // `?tier=0` (to 3) pins the quality tier, for screenshots on slow software renderers.
  const pinned = params.has('tier')
  let tier = pinned ? clamp(Number(params.get('tier')) || 0, 0, 3) : Number(remember('tapiti.walk.tier', undefined, sessionStorage)) || 0
  if (tier >= 3) flames.visible = false
  let dpr = 1
  const applySize = () => {
    const rect = canvas.getBoundingClientRect()
    const cap = innerWidth < 800 ? 1.25 : 1.5
    dpr = tier >= 1 ? 1 : Math.min(devicePixelRatio || 1, cap)
    renderer.setPixelRatio(dpr)
    renderer.setSize(Math.max(1, Math.round(rect.width)), Math.max(1, Math.round(rect.height)), false)
    camera.aspect = rect.width / Math.max(1, rect.height)
    camera.updateProjectionMatrix()
  }
  let lastHeight = innerHeight
  let resizeTimer = 0
  const onResize = () => {
    clearTimeout(resizeTimer)
    resizeTimer = setTimeout(() => {
      stillDrawn = false
      // The mobile URL bar changes the height by a few percent while scrolling: not a resize.
      const heightOnly = Math.abs(innerHeight - lastHeight) / lastHeight < 0.15 && canvas.width === Math.round(canvas.getBoundingClientRect().width * dpr)
      lastHeight = innerHeight
      measure()
      if (!heightOnly) applySize()
      wake()
    }, 100)
  }

  const forward = new THREE.Vector3()
  let lastScroll = 0
  let frameId = 0
  let stopped = false
  let first = true
  const samples = []
  let sampleStart = 0
  let prev = performance.now()
  let stillDrawn = false

  const render = (dt) => {
    const p = poseAt(state.u, state.dive, 0)
    // The head turns into a bend a moment after the path does, instead of snapping at every corner.
    if (yaw === null || stepped) yaw = p.yaw
    else {
      let delta = p.yaw - yaw
      delta -= Math.round(delta / (2 * Math.PI)) * 2 * Math.PI
      yaw += delta * (1 - Math.exp(-Math.min(dt, 100) / 1000 / 0.22))
    }
    const k = 1 - Math.exp(-Math.min(dt, 100) / 1000 / 0.15)
    look.yaw += (-look.x * 0.2 - look.yaw) * k
    look.pitch += (-look.y * 0.1 - look.pitch) * k
    const yawNow = yaw + look.yaw
    const pitchNow = p.pitch + look.pitch
    camera.position.set(p.px, p.py, p.pz)
    forward.set(Math.sin(yawNow) * Math.cos(pitchNow), Math.sin(pitchNow), Math.cos(yawNow) * Math.cos(pitchNow))
    camera.lookAt(p.px + forward.x * 10, p.py + forward.y * 10, p.pz + forward.z * 10)
    camera.fov = innerWidth < innerHeight * 0.8 ? 80 : 70
    camera.updateProjectionMatrix()
    renderer.render(scene, camera)
    if (first) {
      first = false
      mountSigns()
      document.documentElement.classList.add('walk-on')
      measure()
      const linked = sections.find((section) => `#${section.id}` === location.hash)
      if (linked) {
        // Scene sections replace the reading layout: resolve the deep link against their final heights.
        scrollTo({ top: linked.getBoundingClientRect().top + scrollY, behavior: 'instant' })
        want = target()
        state.u = stationU[want.station]
        state.dive = want.dive
        yaw = null
        onScroll()
      }
    }
    placeSigns()
    window.__walk = { tier, calls: renderer.info.render.calls, triangles: renderer.info.render.triangles, stations: built.stations, camera: { x: p.px, y: p.py, z: p.pz, u: state.u }, torches: count, poolSize: 0, lights: 0, stepped }
  }

  const adapt = (now) => {
    const dt = now - prev
    prev = now
    if (stepped || pinned || dt > 200) return
    if (!sampleStart) sampleStart = now
    samples.push(dt)
    if (now - sampleStart >= 2000) {
      samples.sort((a, b) => a - b)
      const p95 = samples[Math.floor(samples.length * 0.95)]
      samples.length = 0
      sampleStart = 0
      if (p95 > 22 && tier < 3) {
        tier++
        remember('tapiti.walk.tier', String(tier), sessionStorage)
        if (tier === 1) applySize()
        else flames.visible = false
      }
    }
  }

  const step = (now) => {
    frameId = 0
    if (stopped || document.hidden || offscreen) return
    want = target()
    if (stepped) want.u = stationU[want.station]
    const dt = Math.min(now - prev, 100)
    const k = stepped ? 1 : 1 - Math.exp(-(dt / 1000) / 0.26)
    state.u += (want.u - state.u) * k
    state.dive += (want.dive - state.dive) * k
    const moving = Math.abs(want.u - state.u) > 1e-4 || Math.abs(want.dive - state.dive) > 1e-4
    if (!moving) { state.u = want.u; state.dive = want.dive }
    adapt(now)
    if (stepped && stillDrawn && !moving && state.station === want.station) return
    state.station = want.station
    render(dt)
    if (stepped) { stillDrawn = true; return }
    if (moving || now - lastScroll < 300) { if (!frameId) frameId = requestAnimationFrame(step); return }
  }
  const wake = () => {
    if (stopped || frameId) return
    frameId = requestAnimationFrame(step)
  }
  const onScroll = () => { lastScroll = performance.now(); wake() }
  addEventListener('scroll', onScroll, { passive: true })
  addEventListener('resize', onResize)
  document.addEventListener('visibilitychange', () => { if (!document.hidden) { prev = performance.now(); wake() } })
  canvas.addEventListener('webglcontextlost', (event) => {
    event.preventDefault()
    stopped = true
    document.documentElement.classList.remove('walk-on')
  })
  new ResizeObserver(onResize).observe(canvas)
  // On narrow screens the scene sits behind the entrance only and scrolls away: no frames while it is out of sight.
  let offscreen = false
  new IntersectionObserver(([entry]) => { offscreen = !entry.isIntersecting; if (!offscreen) wake() }).observe(canvas)

  measure()
  applySize()
  want = target()
  state = { u: stepped ? stationU[want.station] : 0, dive: stepped ? want.dive : 0, station: -1 }
  prev = performance.now()
  lastScroll = prev
  wake()

  const setStepped = (value) => {
    stepped = value
    remember('tapiti.walk.paused', value ? '1' : '0')
    stillDrawn = false
    wake()
  }
  return {
    get stepped() { return stepped },
    pause: () => setStepped(true),
    resume: () => setStepped(reduced()),
    dispose() {
      stopped = true
      cancelAnimationFrame(frameId)
      removeEventListener('scroll', onScroll)
      removeEventListener('resize', onResize)
      layer.remove()
      for (const o of [wallGeometry, plane, pillarGeometry, jambGeometry, lintelGeometry, doorMaterial, bracketGeometry, flameGeometry, stepGeometry, texture, wallMaterial, pillarMaterial, floorMaterial, stepMaterial, bracketMaterial, flames.material]) o.dispose()
      renderer.dispose()
      renderer.forceContextLoss()
      window.__walk = null
    },
  }
}

/* ---------------- The landing page ---------------- */

async function boot() {
  if (!root || params.get('scene') === 'off' || saveData() || matchMedia('(forced-colors: active)').matches || !hasWebGL2()) return
  const canvas = root.querySelector('canvas')
  const seed = Number(root.dataset.seed)
  const size = Number(root.dataset.size)
  const [wasm, build, THREE, paper] = await Promise.all([import('/lab/pkg/tapiti.js'), import(root.dataset.build), import(root.dataset.three), import(root.dataset.paper)])
  await wasm.default()
  const answer = JSON.parse(wasm.stage_json(seed, wasm.rich_params(size)))
  const floor = answer.floor
  const built = build.build(floor)
  const handle = mount({ THREE, paper, canvas, floor, scene: built, page: document.querySelector('.rooms') })

  const toggle = document.querySelector('.route-pause')
  if (toggle) {
    toggle.hidden = false
    const label = () => {
      const on = handle.stepped
      toggle.setAttribute('aria-pressed', String(on))
      toggle.textContent = on ? toggle.dataset.resume : toggle.dataset.pause
    }
    label()
    toggle.addEventListener('click', () => { handle.stepped ? handle.resume() : handle.pause(); label() })
  }
  addEventListener('pagehide', (event) => { if (!event.persisted) handle.dispose() })
}

if (root) {
  const start = () => boot().catch((error) => console.warn('dungeon: the scene did not start, the map stays', error))
  const idle = () => (window.requestIdleCallback ? requestIdleCallback(start, { timeout: 2000 }) : setTimeout(start, 200))
  document.readyState === 'complete' ? idle() : addEventListener('load', idle, { once: true })
}
