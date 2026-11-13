// The dungeon: a fixed three.js scene behind the landing page, seen in the first person. Scrolling walks the camera, at eye height, along the
// main path of one real floor (the one the generator draws for `data-seed` and `data-size`, built here by the same wasm module the lab uses).
// The rooms are the generator's own; only the drawing is a dungeon: stone courses on the walls, flagged floor, a low ceiling, torchlight and fog.
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

/** A number in [0, 1) from an integer, with no randomness: every visitor sees the same flicker. */
const hash = (n) => (Math.imul(n + 1, 2654435761) >>> 0) / 4294967296

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

/* ---------------- Light, flames ---------------- */

const FLAME_VERTEX = `
  attribute float phase;
  attribute float lit;
  uniform float uTime;
  uniform float uSway;
  varying vec2 vUv;
  varying float vLevel;
  void main() {
    vUv = uv;
    vec4 mv = modelViewMatrix * instanceMatrix * vec4(0.0, 0.0, 0.0, 1.0);
    float n = (sin(uTime * 7.1 + phase * 6.2831) + sin(uTime * 11.3 + phase * 17.0) + sin(uTime * 17.9 + phase * 31.0)) / 3.0;
    vLevel = lit * (0.86 + 0.14 * n * uSway);
    mv.xy += position.xy * vec2(7.0, 13.0) * (0.9 + 0.1 * vLevel);
    mv.y += 6.0;
    gl_Position = projectionMatrix * mv;
  }
`
const FLAME_FRAGMENT = `
  uniform vec3 uColor;
  varying vec2 vUv;
  varying float vLevel;
  void main() {
    vec2 p = vUv - vec2(0.5, 0.32);
    p.x *= 1.7;
    float d = length(p) * 2.0;
    float a = smoothstep(1.0, 0.0, d);
    a *= a;
    vec3 core = mix(uColor, vec3(1.0, 0.86, 0.62), smoothstep(0.5, 0.0, d));
    gl_FragColor = vec4(core * a * vLevel, a * vLevel);
  }
`

/* ---------------- Stone ---------------- */

/**
 * Lambert lighting with courses of dressed stone, drawn from the wall's own coordinates (`uv`: length along the wall, height): a row every
 * 8 units, blocks 18 long, each row shifted, each block a little lighter or darker, mortar between. No texture to download.
 */
function stone(THREE, color) {
  const material = new THREE.MeshLambertMaterial({ color, vertexColors: true })
  material.onBeforeCompile = (shader) => {
    shader.vertexShader = shader.vertexShader
      .replace('void main() {', 'varying vec2 vStoneUv;\nvoid main() {')
      .replace('#include <begin_vertex>', '#include <begin_vertex>\nvStoneUv = uv;')
    shader.fragmentShader = shader.fragmentShader
      .replace(
        'void main() {',
        `varying vec2 vStoneUv;
        float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
        void main() {`,
      )
      .replace(
        '#include <color_fragment>',
        `#include <color_fragment>
        vec2 g = vec2(vStoneUv.x / 18.0, vStoneUv.y / 8.0);
        float row = floor(g.y);
        g.x += row * 0.5 + h21(vec2(row, 1.0)) * 3.0;
        vec2 cell = floor(g);
        vec2 f = fract(g);
        float mortar = clamp(step(f.x, 0.035) + step(f.y, 0.075), 0.0, 1.0);
        float tone = 0.72 + 0.36 * h21(cell + row * 7.0);
        diffuseColor.rgb *= mix(tone, 0.42, mortar);`,
      )
  }
  material.customProgramCacheKey = () => 'tapiti-stone'
  return material
}

/** One canvas for the floor and the ceiling: flagstone joints along the floor's grid, the mask as alpha, so one plane takes the floor's shape. */
function flagTexture(THREE, floor, scene, size) {
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
  c.fillStyle = '#6d675f'
  c.fillRect(0, 0, W, H)
  c.globalCompositeOperation = 'source-atop'
  // Flags: a row every 18 units, a joint every 30, each row shifted; a few darker flags from a fixed hash.
  const flagW = 30 * sx
  const flagH = 18 * sy
  for (let row = 0, y = 0; y < H; row++, y += flagH) {
    for (let col = -1, x = -((row * 0.5 + 0.3) % 1) * flagW; x < W; col++, x += flagW) {
      const v = hash(row * 997 + col)
      c.globalAlpha = 0.22 * v
      c.fillStyle = '#000'
      c.fillRect(x, y, flagW, flagH)
    }
  }
  c.globalAlpha = 0.7
  c.strokeStyle = '#1f1c19'
  c.lineWidth = 1.4
  for (let row = 0, y = 0; y < H; row++, y += flagH) {
    c.beginPath()
    c.moveTo(0, y)
    c.lineTo(W, y)
    for (let x = -((row * 0.5 + 0.3) % 1) * flagW; x < W; x += flagW) { c.moveTo(x, y); c.lineTo(x, y + flagH) }
    c.stroke()
  }
  c.globalAlpha = 1
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  texture.anisotropy = 8
  return texture
}

/* ---------------- Mount ---------------- */

/** Builds the scene for `floor` on `canvas` and returns a handle. `THREE` is the vendored module, `scene` the result of build.js. */
export function mount({ THREE, canvas, floor, scene: built, page }) {
  const abyss = new THREE.Color(css('--abyss') || '#0b0e12')
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: devicePixelRatio < 1.5, powerPreference: 'low-power' })
  renderer.outputColorSpace = THREE.SRGBColorSpace
  renderer.toneMapping = THREE.ACESFilmicToneMapping
  renderer.toneMappingExposure = 1.25
  renderer.setClearColor(abyss, 1)

  const scene = new THREE.Scene()
  scene.background = abyss
  scene.fog = new THREE.FogExp2(abyss, 0.0045)
  const camera = new THREE.PerspectiveCamera(70, 1, 2, 2500)

  /* Walls: stone courses, with the vertical blocks of the wall's own length. */
  const wallGeometry = new THREE.BufferGeometry()
  const w = built.walls
  wallGeometry.setAttribute('position', new THREE.BufferAttribute(w.positions, 3))
  wallGeometry.setAttribute('normal', new THREE.BufferAttribute(w.normals, 3))
  wallGeometry.setAttribute('uv', new THREE.BufferAttribute(w.uvs, 2))
  const colors = new Float32Array(w.shade.length * 3)
  for (let i = 0; i < w.shade.length; i++) colors.fill(w.shade[i], i * 3, i * 3 + 3)
  wallGeometry.setAttribute('color', new THREE.BufferAttribute(colors, 3))
  wallGeometry.setIndex(new THREE.BufferAttribute(w.indices, 1))
  const wallMaterial = stone(THREE, '#9a9aa2')
  scene.add(new THREE.Mesh(wallGeometry, wallMaterial))

  /* Floor and ceiling: one plane each, the texture's alpha cuts it to the shape of the floor. */
  const [bx0, by0, bx1, by1] = built.bounds
  const texture = flagTexture(THREE, floor, built, 2048)
  const plane = new THREE.PlaneGeometry(bx1 - bx0, by1 - by0)
  const floorMaterial = new THREE.MeshLambertMaterial({ map: texture, alphaTest: 0.5 })
  const ground = new THREE.Mesh(plane, floorMaterial)
  ground.rotation.x = -Math.PI / 2
  ground.position.set((bx0 + bx1) / 2, 0, (by0 + by1) / 2)
  scene.add(ground)
  const ceilingMaterial = new THREE.MeshLambertMaterial({ map: texture, alphaTest: 0.5, color: '#8a847b' })
  const ceiling = new THREE.Mesh(plane, ceilingMaterial)
  ceiling.rotation.x = Math.PI / 2
  ceiling.position.set((bx0 + bx1) / 2, WALL, (by0 + by1) / 2)
  scene.add(ceiling)

  /* Corner pillars at the junctions hide the seams where corridors cross. */
  const pillars = built.pillars.length / 2
  const pillarMaterial = stone(THREE, '#8a8a92')
  const pillarGeometry = new THREE.BoxGeometry(7, WALL, 7)
  // Box faces carry uv in [0, 1]; scale them to wall units so the courses match the walls.
  const boxUv = pillarGeometry.attributes.uv
  for (let i = 0; i < boxUv.count; i++) boxUv.setXY(i, boxUv.getX(i) * 7, boxUv.getY(i) * WALL)
  const pillarColors = new Float32Array(pillarGeometry.attributes.position.count * 3).fill(1)
  pillarGeometry.setAttribute('color', new THREE.BufferAttribute(pillarColors, 3))
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
    g.setAttribute('color', new THREE.BufferAttribute(new Float32Array(g.attributes.position.count * 3).fill(1), 3))
  }
  const doorMaterial = stone(THREE, '#7d7d86')
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

  /* The stairwell: a square cut into the floor, steps going down, and cold light rising from it. */
  const stairs = built.stairs
  const side = Math.min(stairs.radius * 1.1, 110)
  const stepMaterial = new THREE.MeshLambertMaterial({ color: '#615b52' })
  const stepGeometry = new THREE.BoxGeometry(side, 20, side / 8)
  const steps = []
  for (let k = 0; k < 8; k++) {
    const step = new THREE.Mesh(stepGeometry, stepMaterial)
    step.position.set(stairs.center[0], -10 - k * 20, stairs.center[1] - side / 2 + (k + 0.5) * (side / 8))
    steps.push(step)
    scene.add(step)
  }
  const below = new THREE.PointLight(css('--light-below') || '#6d8fc4', 400, 380, 1)
  below.position.set(stairs.center[0], -90, stairs.center[1])
  scene.add(below)

  /* Torches: brackets and flames (two draw calls), lit by a fixed pool of lights that follows the camera. */
  const count = built.torches.length / 4
  const bracketGeometry = new THREE.BoxGeometry(3, 12, 3)
  const bracketMaterial = new THREE.MeshLambertMaterial({ color: '#3a3430' })
  const brackets = new THREE.InstancedMesh(bracketGeometry, bracketMaterial, count)
  const flameGeometry = new THREE.PlaneGeometry(1, 1)
  const flames = new THREE.InstancedMesh(flameGeometry, null, count)
  const phase = new Float32Array(count)
  const lit = new Float32Array(count)
  const m4 = new THREE.Matrix4()
  const spots = []
  for (let i = 0; i < count; i++) {
    const x = built.torches[i * 4]
    const z = built.torches[i * 4 + 1]
    spots.push([x, 38, z])
    brackets.setMatrixAt(i, m4.makeTranslation(x, 31, z))
    flames.setMatrixAt(i, m4.makeTranslation(x, 38, z))
    phase[i] = hash(i)
  }
  flames.geometry.setAttribute('phase', new THREE.InstancedBufferAttribute(phase, 1))
  const litAttribute = new THREE.InstancedBufferAttribute(lit, 1)
  flames.geometry.setAttribute('lit', litAttribute)
  const flameUniforms = { uTime: { value: 0 }, uSway: { value: 1 }, uColor: { value: new THREE.Color(css('--light-torch') || '#ff8a4c') } }
  flames.material = new THREE.ShaderMaterial({ vertexShader: FLAME_VERTEX, fragmentShader: FLAME_FRAGMENT, uniforms: flameUniforms, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending })
  flames.frustumCulled = false
  scene.add(brackets, flames)
  // Torches light up outwards from the entrance: delay by distance from the first station.
  const entrance = built.stations[0]
  let farthest = 1
  const delays = spots.map(([x, , z]) => { const d = Math.sqrt((x - entrance.x) ** 2 + (z - entrance.y) ** 2); farthest = Math.max(farthest, d); return d })
  for (let i = 0; i < count; i++) delays[i] = (delays[i] / farthest) * 0.8

  /* Lights: warm torches against dark stone, and a little light about the one who walks. */
  // Decay 1, not the physical 2: wide soft pools of light instead of hot spots on the nearest wall and black beyond.
  const TORCH = 150
  const CARRIED = 70
  const hemisphere = new THREE.HemisphereLight(new THREE.Color(css('--light-fill') || '#2b3442'), abyss, 0.9)
  scene.add(hemisphere)
  const carried = new THREE.PointLight(css('--light-carried') || '#ffd4ad', CARRIED, 300, 1)
  scene.add(carried)
  const pool = []
  const POOL = 6
  for (let i = 0; i < POOL; i++) {
    const light = new THREE.PointLight(css('--light-torch') || '#ff8a4c', TORCH, 360, 1)
    pool.push(light)
    scene.add(light)
  }
  let poolSize = POOL

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
  // Where each room's text stands, as a share of the width to move the picture away from it (+ right, - left). Matches landing.css.
  const SHIFT = [0.14, 0.16, 0, 0, -0.14, 0.16, -0.05]
  const shiftAt = (u) => {
    for (let i = 0; i < stationU.length - 1; i++) if (u <= stationU[i + 1]) return lerp(SHIFT[i], SHIFT[i + 1], smooth(stationU[i], stationU[i + 1], u))
    return SHIFT[SHIFT.length - 1]
  }

  const here = [0, 0, 0]
  const ahead = [0, 0, 0]
  const pose = { px: 0, py: EYE, pz: 0, yaw: 0, pitch: 0 }
  let yaw = null
  const headingAt = (u) => {
    pointAt(u, here)
    // Look at a point a little way along the path; where it turns, the head follows a moment later (see `yaw` in render).
    pointAt(u + LOOK / total, ahead)
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

  /* Scroll to walk position */
  const sections = page ? [...page.querySelectorAll('[data-station]')] : []
  const footer = document.querySelector('.site-footer')
  let tops = []
  let heights = []
  const measure = () => {
    const base = scrollY
    tops = sections.map((s) => s.getBoundingClientRect().top + base)
    heights = sections.map((s) => s.getBoundingClientRect().height)
  }
  const target = () => {
    if (!sections.length) return { u: 0, dive: 0, station: 0 }
    const vh = innerHeight
    const mid = scrollY + vh * 0.5
    let i = 0
    for (let k = 0; k < sections.length; k++) if (tops[k] <= mid) i = k
    const f = clamp((mid - tops[i]) / (heights[i] || 1), 0, 1)
    const u = i >= stationU.length - 1 ? 1 : lerp(stationU[i], stationU[i + 1], smooth(0.35, 1, f))
    let dive = 0
    if (footer) {
      const rect = footer.getBoundingClientRect()
      dive = clamp((vh - rect.top) / Math.max(1, Math.min(rect.height, vh * 0.6)), 0, 1)
    }
    return { u, dive, station: f > 0.65 && i < stationU.length - 1 ? i + 1 : i }
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
  if (tier >= 2) poolSize = Math.min(poolSize, 3)
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

  const minimapSvg = document.querySelector('.minimap-floor svg')
  const you = document.querySelector('.minimap .you')
  const viewBox = minimapSvg ? minimapSvg.viewBox.baseVal : null
  const stops = [...document.querySelectorAll('.minimap .stop')]
  const ticks = [...document.querySelectorAll('.route-list a')]
  let currentStation = -1
  const markStation = (i) => {
    if (i === currentStation) return
    currentStation = i
    for (const [k, el] of stops.entries()) k === i ? el.setAttribute('aria-current', 'true') : el.removeAttribute('aria-current')
    for (const [k, el] of ticks.entries()) k === i ? el.setAttribute('aria-current', 'true') : el.removeAttribute('aria-current')
  }

  const ordered = new Array(count)
  const scratch = new Float32Array(count)
  const forward = new THREE.Vector3()
  let started = performance.now()
  let lastScroll = 0
  let frameId = 0
  let timer = 0
  let stopped = false
  let first = true
  const samples = []
  let sampleStart = 0
  let prev = performance.now()
  let stillDrawn = false
  let walked = 0

  const render = (now, dt) => {
    const t = (now - started) / 1000
    const ignition = stepped ? 1 : null
    // Eyes: a slow sway of the head with the distance walked, none when the page stands still or motion is reduced.
    const bob = stepped ? 0 : Math.sin(walked * 0.32) * 0.7
    const p = poseAt(state.u, state.dive, bob)
    // The head turns into a bend a moment after the path does, instead of snapping at every corner.
    if (yaw === null || stepped) yaw = p.yaw
    else {
      let delta = p.yaw - yaw
      delta -= Math.round(delta / (2 * Math.PI)) * 2 * Math.PI
      yaw += delta * (1 - Math.exp(-Math.min(dt, 100) / 1000 / 0.22))
    }
    camera.position.set(p.px, p.py, p.pz)
    forward.set(Math.sin(yaw) * Math.cos(p.pitch), Math.sin(p.pitch), Math.cos(yaw) * Math.cos(p.pitch))
    camera.lookAt(p.px + forward.x * 10, p.py + forward.y * 10, p.pz + forward.z * 10)
    camera.fov = innerWidth < innerHeight * 0.8 ? 80 : 70
    // On wide screens the text stands at one side: the picture moves to the other, without turning the camera.
    const cw = canvas.width
    const ch = canvas.height
    const shift = innerWidth >= 1100 ? shiftAt(state.u) * (1 - state.dive) : 0
    if (shift) camera.setViewOffset(cw, ch, -shift * cw, 0, cw, ch)
    else camera.clearViewOffset()
    camera.updateProjectionMatrix()
    // Lights: the pool goes to the torches nearest to the eyes.
    for (let i = 0; i < count; i++) scratch[i] = (spots[i][0] - p.px) ** 2 + (spots[i][2] - p.pz) ** 2
    for (let i = 0; i < count; i++) ordered[i] = i
    ordered.sort((a, b) => scratch[a] - scratch[b])
    let changed = false
    for (let i = 0; i < count; i++) {
      const level = ignition ?? smooth(0, 0.45, t - delays[i])
      if (lit[i] !== level) { lit[i] = level; changed = true }
    }
    if (changed) litAttribute.needsUpdate = true
    flameUniforms.uTime.value = stepped ? 0 : t
    flameUniforms.uSway.value = stepped ? 0 : 1
    for (let k = 0; k < POOL; k++) {
      const light = pool[k]
      if (k >= poolSize || !count) { light.intensity = 0; continue }
      const torch = ordered[k]
      const [x, y, z] = spots[torch]
      light.position.set(x, y, z)
      const n = stepped ? 0 : (Math.sin(t * 7.1 + hash(torch) * 6.28) + Math.sin(t * 11.3 + hash(torch) * 17) + Math.sin(t * 17.9 + hash(torch) * 31)) / 3
      light.intensity = TORCH * (0.86 + 0.14 * n) * lit[torch]
    }
    carried.position.set(p.px + forward.x * 12, Math.max(p.py - 4, -60), p.pz + forward.z * 12)
    carried.intensity = CARRIED * smooth(0, 0.45, t)
    below.intensity = 400 * smooth(0.6, 1, state.u) * (1 - state.dive * 0.4)
    renderer.render(scene, camera)
    if (first) {
      first = false
      document.documentElement.classList.add('walk-on')
    }
    // The minimap's "you are here" dot and the current station.
    if (you && viewBox) {
      you.style.left = `${((here[0] - viewBox.x) / viewBox.width) * 100}%`
      you.style.top = `${((here[2] - viewBox.y) / viewBox.height) * 100}%`
    }
    markStation(want.station)
    window.__walk = { tier, calls: renderer.info.render.calls, triangles: renderer.info.render.triangles, stations: built.stations, camera: { x: p.px, y: p.py, z: p.pz, u: state.u }, torches: count, poolSize, lights: pool.length + 3, stepped }
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
        else if (tier === 2) poolSize = Math.min(poolSize, 3)
        else flames.visible = false
      }
    }
  }

  const step = (now) => {
    frameId = 0
    timer = 0
    if (stopped || document.hidden || offscreen) return
    want = target()
    if (stepped) want.u = stationU[want.station]
    const dt = Math.min(now - prev, 100)
    const k = stepped ? 1 : 1 - Math.exp(-(dt / 1000) / 0.18)
    const before = state.u
    state.u += (want.u - state.u) * k
    state.dive += (want.dive - state.dive) * k
    walked += Math.abs(state.u - before) * total
    const moving = Math.abs(want.u - state.u) > 1e-4 || Math.abs(want.dive - state.dive) > 1e-4
    if (!moving) { state.u = want.u; state.dive = want.dive }
    adapt(now)
    if (stepped && stillDrawn && !moving && state.station === want.station) return
    state.station = want.station
    render(now, dt)
    const ignitionBusy = now - started < 1400
    if (stepped) { stillDrawn = true; return }
    if (moving || ignitionBusy || now - lastScroll < 300) { frameId = requestAnimationFrame(step); return }
    if (state.dive >= 1) return
    // Resting: the flames still flicker, at 15 fps, for a few seconds, then the loop sleeps until the next scroll.
    if (now - lastScroll < 6000) timer = setTimeout(() => { frameId = requestAnimationFrame(step) }, 66)
  }
  const wake = () => {
    if (stopped || frameId || timer) return
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
  started = performance.now()
  prev = started
  lastScroll = started
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
      clearTimeout(timer)
      cancelAnimationFrame(frameId)
      removeEventListener('scroll', onScroll)
      removeEventListener('resize', onResize)
      for (const o of [wallGeometry, plane, pillarGeometry, jambGeometry, lintelGeometry, doorMaterial, bracketGeometry, flameGeometry, stepGeometry, texture, wallMaterial, pillarMaterial, floorMaterial, ceilingMaterial, stepMaterial, bracketMaterial, flames.material]) o.dispose()
      renderer.dispose()
      renderer.forceContextLoss()
      window.__walk = null
    },
  }
}

/* ---------------- The landing page ---------------- */

async function boot() {
  if (!root || params.get('scene') === 'off' || saveData() || !hasWebGL2()) return
  const canvas = root.querySelector('canvas')
  const seed = Number(root.dataset.seed)
  const size = Number(root.dataset.size)
  const [wasm, build, THREE] = await Promise.all([import('/lab/pkg/tapiti.js'), import(root.dataset.build), import(root.dataset.three)])
  await wasm.default()
  const answer = JSON.parse(wasm.stage_json(seed, wasm.rich_params(size)))
  const floor = answer.floor
  const built = build.build(floor)
  const handle = mount({ THREE, canvas, floor, scene: built, page: document.querySelector('.rooms') })

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
