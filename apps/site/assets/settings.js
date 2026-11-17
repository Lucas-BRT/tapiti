// The site's settings: theme, text size, motion, and, for the lab, the reading panel, the floors kept for undo and the single-key
// shortcuts. Each is a set of presets and, where it makes sense, a value of the user's own, as in the desktop app. They are kept in this
// browser, applied at once, and start from what the system says. The lab reads them through `resolve()`.

const KEY = 'tapiti.settings'
const THEME_KEY = 'tapiti-theme' // kept apart too, because theme.js reads it in the head before the page is painted

const DEFAULTS = {
  theme: 'system',
  text: { preset: 'normal', value: 100 },
  motion: { preset: null, value: 120 }, // null: what the system asks for
  reveal: { preset: null, value: 600 },
  reading: 'automatic',
  width: { preset: 'normal', value: 328 },
  memory: { preset: 'small', value: 60 },
  shortcuts: 'on',
}
const TEXT = { small: 90, normal: 100, large: 115, larger: 130 }
const MOTION = { off: 0, fast: 80, normal: 120, slow: 220 }
const REVEAL = { off: 0, fast: 300, normal: 600, slow: 1200 }
const WIDTH = { narrow: 280, normal: 328, wide: 400 }

const read = () => {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? '{}')
    return { ...structuredClone(DEFAULTS), ...saved }
  } catch {
    return structuredClone(DEFAULTS)
  }
}
const write = (settings) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(settings))
    localStorage.setItem(THEME_KEY, settings.theme)
  } catch { /* the choice applies to this visit only */ }
}
const wantsLessMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches

// What the options mean in numbers, with the system's wish as the first answer to motion.
export const resolve = (settings = read()) => {
  const motionMs = settings.motion.preset === null ? (wantsLessMotion() ? 0 : MOTION.normal) : settings.motion.preset === 'custom' ? settings.motion.value : MOTION[settings.motion.preset]
  const revealMs = settings.reveal.preset === null ? (wantsLessMotion() ? 0 : REVEAL.normal) : settings.reveal.preset === 'custom' ? settings.reveal.value : REVEAL[settings.reveal.preset]
  const memoryRooms = { off: 0, small: 60, always: Infinity }[settings.memory.preset] ?? settings.memory.value
  return {
    theme: settings.theme,
    textPercent: settings.text.preset === 'custom' ? settings.text.value : TEXT[settings.text.preset] ?? 100,
    motionMs,
    revealMs,
    reading: settings.reading,
    readingWidth: settings.width.preset === 'custom' ? settings.width.value : WIDTH[settings.width.preset] ?? 328,
    memoryRooms,
    shortcuts: settings.shortcuts !== 'off',
  }
}

const apply = () => {
  const s = resolve()
  const html = document.documentElement
  if (s.theme === 'system') delete html.dataset.theme
  else html.dataset.theme = s.theme
  // The text size scales the whole page, as the browser's own zoom does; the plan in the lab is drawn from the page, so it follows.
  html.style.zoom = s.textPercent === 100 ? '' : String(s.textPercent / 100)
  html.style.setProperty('--t', `${s.motionMs}ms`)
  html.dataset.motion = s.motionMs === 0 ? 'off' : 'on'
}

const dialog = document.getElementById('settings-dialog')
const button = document.querySelector('.settings-button')

const choiceInputs = (option) => [...dialog.querySelectorAll(`input[type=radio][name="${option}"]`)]
const customBox = (option) => dialog.querySelector(`[data-option="${option}"] .setting-custom`)
const customInput = (option) => dialog.querySelector(`input[data-custom="${option}"]`)

const OPTIONS = { theme: 'theme', text: 'text', motion: 'motion', reveal: 'reveal', reading: 'reading', width: 'width', memory: 'memory', shortcuts: 'shortcuts' }
const hasCustom = (option) => option === 'text' || option === 'motion' || option === 'reveal' || option === 'width' || option === 'memory'

const show = () => {
  const s = read()
  for (const option of Object.keys(OPTIONS)) {
    const value = hasCustom(option) ? s[option].preset : s[option]
    const chosen = hasCustom(option) && value === null ? (wantsLessMotion() ? 'off' : 'normal') : value
    for (const input of choiceInputs(option)) input.checked = input.value === chosen
    if (hasCustom(option)) {
      const box = customBox(option)
      box.hidden = chosen !== 'custom'
      customInput(option).value = String(s[option].value)
    }
  }
}

const save = () => {
  const s = read()
  for (const option of Object.keys(OPTIONS)) {
    const picked = choiceInputs(option).find((input) => input.checked)?.value
    if (!picked) continue
    if (hasCustom(option)) {
      const raw = Number(customInput(option).value)
      const input = customInput(option)
      const value = Number.isFinite(raw) ? Math.min(Number(input.max), Math.max(Number(input.min), Math.round(raw))) : s[option].value
      s[option] = { preset: picked, value }
    } else s[option] = picked
  }
  write(s)
  show()
  apply()
  window.dispatchEvent(new Event('themechange'))
  window.dispatchEvent(new Event('settingschange'))
}

// Everywhere but the lab, the header has only a theme button: system, light, dark, and back.
const toggle = document.querySelector('.theme-toggle')
if (toggle) {
  const order = ['system', 'light', 'dark']
  const names = { system: toggle.dataset.system, light: toggle.dataset.light, dark: toggle.dataset.dark }
  const show = () => { toggle.textContent = `${toggle.dataset.label}: ${names[read().theme] ?? names.system}` }
  toggle.hidden = false
  show()
  toggle.addEventListener('click', () => {
    const s = read()
    s.theme = order[(order.indexOf(s.theme) + 1) % order.length]
    write(s)
    apply()
    show()
    window.dispatchEvent(new Event('themechange'))
  })
}

if (dialog && button) {
  button.hidden = false
  button.addEventListener('click', () => { show(); dialog.showModal() })
  dialog.addEventListener('change', save)
  dialog.addEventListener('input', (event) => { if (event.target.matches('input[type=number]')) save() })
  dialog.querySelector('#settings-reset')?.addEventListener('click', () => {
    try { localStorage.removeItem(KEY); localStorage.removeItem(THEME_KEY) } catch { /* nothing kept */ }
    show()
    apply()
    window.dispatchEvent(new Event('themechange'))
    window.dispatchEvent(new Event('settingschange'))
  })
  // A click on the backdrop closes the dialog, as Esc does.
  dialog.addEventListener('click', (event) => { if (event.target === dialog) dialog.close() })
  dialog.addEventListener('close', () => button.focus())
}
apply()
matchMedia('(prefers-reduced-motion: reduce)').addEventListener('change', apply)
