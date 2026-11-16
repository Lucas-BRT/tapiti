# fonts

The desktop app is set in IBM Plex, embedded as TrueType and loaded in `theme.rs` (`install_fonts`). SIL Open Font License 1.1: the two
licence texts are next to the files.

| file | use | licence | source (github.com/google/fonts) |
|---|---|---|---|
| `IBMPlexMono-Regular.ttf` | all text and numbers (`FontFamily::Proportional` and `Monospace`) | `OFL-IBMPlexMono.txt` | `ofl/ibmplexmono` at `431022a4eeb0e1009ba0126f41353d8ca2375a2a` |
| `IBMPlexMono-Medium.ttf` | values and changed counts (`theme::medium()`) | `OFL-IBMPlexMono.txt` | the same |
| `IBMPlexSansCondensed-SemiBold.ttf` | group titles and the brand (`theme::display()`) | `OFL-IBMPlexSansCondensed.txt` | `ofl/ibmplexsanscondensed` at `e727fafe8e8a5d518972286d9763a32fdfbe2b91` |

sha256:

```
6a3412f058c7d8dfd9170c41e85ade48e5156ecb89356110ca57a0a27734af46  IBMPlexMono-Regular.ttf
a9b4c49bb299e05b5f6c481e7fb5e78943d2793249a0c8874ab574a2d1ea6755  IBMPlexMono-Medium.ttf
a081ca60548aea14100757a2d6fe71ca60fd90ebce779c9a8ae5a0c8fb4526d4  IBMPlexSansCondensed-SemiBold.ttf
```

egui's own faces stay behind Plex in each family, for the few glyphs Plex Mono does not have. Plex Mono has no triangles, so the carets of
the groups are drawn, not typed.
