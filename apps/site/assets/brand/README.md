# Brand assets

- `logo-sketch.png` (1000×1000): the hand-drawn rabbit head, the source of the logo.
- `logo.svg`: the logo, the sketch traced to centre lines (skeleton of the strokes, gaps closed, Gaussian σ = 6 points to remove hand tremor, cubic Béziers), drawn as 36-unit round strokes in `#ff2e63` and cropped to the drawing. It has no background, so it works on dark and light pages. Used by the site header and footer, and by the README.
- `icon.svg`: the app icon: the head in cream (`#fff4ec`) on a rounded pink tile (`#ff2e63`), simplified to the outline and the pupil (the whiskers and the eye ring blur at small sizes), with a thicker line. A black square looked like a hole next to the other icons of a launcher, so the tile is rounded, with transparent corners in the raster icons, and pink like the logo. It is the SVG favicon and the source of the raster icons.
- `favicon.ico` (16, 32 and 48 px, rounded), `apple-touch-icon.png` (180 px, a full square: iOS rounds it itself), `../../../desktop/assets/icon.png` (256 px, the desktop window icon and the Linux launcher icon), `icon.ico` (16 to 256 px, embedded in `tapiti-desktop.exe` by `apps/desktop/build.rs`) and `icon.icns` (128 to 1024 px, in `Tapiti.app`): renders of `icon.svg`.
- `og.png`: the 1200×630 social card: the landing page's bench (seed 279, medium, with its rulers in cells) in the dark theme, beside the name and one line, in IBM Plex. Made once from a small HTML page in headless Firefox (`--screenshot --window-size=1200,630`); remake it when the description or the floor palette changes.

The logo is the only pink on the site and in the apps; the interface itself uses the tokens of `../styles.css` (and `theme.rs` in the desktop app).
