# Orbit palette perceptual review (2026-07-23)

Generated from `xtask/src/manifest.rs` by `cargo run -p xtask -- orbit-palette-report --out docs/orbit-palette-review-2026-07-23.md`.

The binding normal-vision gates are exact 65/65 RGB uniqueness, CIE76 ΔE ≥ 4 for every pair, and CIE76 ΔE ≥ 25 between planets. Simulated convergence is review input rather than a failure: category width and accessible body labels remain the primary redundant cues.

- Minimum all-body CIE76: **4.33** (`rhea` / `dysnomia`)
- Minimum planet CIE76: **29.58** (`earth` / `neptune`)
- Lowest base-color contrast against black: **2.88:1** (`bennu`)
- Protanopia simulated minimum: **0.47** (`uranus` / `tempel_1`)
- Deuteranopia simulated minimum: **0.87** (`nix` / `tempel_1`)
- Tritanopia simulated minimum: **1.42** (`enceladus` / `hiiaka`)

| Body | Category | Orbit RGB | Contrast | Nearest normal-vision pair ΔE |
|---|---|---:|---:|---:|
| Mercury | Planet | `#A8A8A8` | 8.83:1 | charon (5.04) |
| Venus | Planet | `#EDC24F` | 12.43:1 | io (7.37) |
| Earth | Planet | `#4D8DFF` | 6.57:1 | hale_bopp (23.60) |
| Mars | Planet | `#D94835` | 4.93:1 | amalthea (25.80) |
| Jupiter | Planet | `#C4854F` | 6.82:1 | eros (12.73) |
| Saturn | Planet | `#F2E3AE` | 16.37:1 | europa (5.77) |
| Uranus | Planet | `#72D4DC` | 12.17:1 | encke (7.82) |
| Neptune | Planet | `#3C55E0` | 3.57:1 | earth (29.58) |
| Moon | Moon | `#C7C2B8` | 11.84:1 | ceres (5.02) |
| Phobos | Moon | `#8F7A68` | 5.15:1 | ganymede (6.68) |
| Deimos | Moon | `#B6A58A` | 8.74:1 | vesta (13.19) |
| Io | Moon | `#F3D35C` | 14.28:1 | venus (7.37) |
| Europa | Moon | `#E8D6A9` | 14.62:1 | saturn (5.77) |
| Ganymede | Moon | `#9C8065` | 5.69:1 | hyperion (5.14) |
| Callisto | Moon | `#6F625A` | 3.57:1 | apophis (5.66) |
| Amalthea | Moon | `#B5534D` | 4.31:1 | gonggong (5.37) |
| Himalia | Moon | `#85817A` | 5.42:1 | oberon (4.98) |
| Mimas | Moon | `#F0E6CE` | 16.92:1 | eris (9.66) |
| Enceladus | Moon | `#D7F2FF` | 18.03:1 | haumea (6.12) |
| Tethys | Moon | `#CEDCE6` | 15.00:1 | dione (4.90) |
| Dione | Moon | `#C4CED8` | 13.17:1 | tethys (4.90) |
| Rhea | Moon | `#A6B5C2` | 10.01:1 | dysnomia (4.33) |
| Titan | Moon | `#D9A43B` | 9.32:1 | venus (11.73) |
| Hyperion | Moon | `#A88665` | 6.26:1 | ganymede (5.14) |
| Iapetus | Moon | `#806A52` | 4.10:1 | phobos (7.46) |
| Phoebe | Moon | `#686A70` | 3.88:1 | umbriel (7.90) |
| Miranda | Moon | `#D5D1C9` | 13.79:1 | moon (5.51) |
| Ariel | Moon | `#C8E1E4` | 15.35:1 | tethys (5.45) |
| Umbriel | Moon | `#7A7E82` | 5.13:1 | oberon (6.23) |
| Titania | Moon | `#B9C4C6` | 11.77:1 | dione (5.79) |
| Oberon | Moon | `#8C8A89` | 6.11:1 | himalia (4.98) |
| Triton | Moon | `#C9B7C4` | 11.05:1 | hydra (6.16) |
| Nereid | Moon | `#8DA1AF` | 7.85:1 | psyche (5.34) |
| Proteus | Moon | `#596477` | 3.51:1 | phoebe (8.86) |
| Ceres | Dwarf Planet | `#B9B7B2` | 10.48:1 | moon (5.02) |
| Pluto | Dwarf Planet | `#D5A58A` | 9.58:1 | deimos (13.32) |
| Eris | Dwarf Planet | `#E8E5DF` | 16.70:1 | miranda (7.17) |
| Haumea | Dwarf Planet | `#DDECF0` | 17.33:1 | hiiaka (5.04) |
| Makemake | Dwarf Planet | `#B86E52` | 5.39:1 | quaoar (13.22) |
| Gonggong | Dwarf Planet | `#A64A48` | 3.69:1 | amalthea (5.37) |
| Quaoar | Dwarf Planet | `#A87363` | 5.30:1 | juno (10.33) |
| Orcus | Dwarf Planet | `#8C9199` | 6.63:1 | oberon (6.17) |
| Sedna | Dwarf Planet | `#8F3E43` | 2.94:1 | gonggong (9.02) |
| Charon | Moon | `#9EA9A4` | 8.67:1 | mercury (5.04) |
| Nix | Moon | `#B5C6DB` | 12.07:1 | dione (6.98) |
| Hydra | Moon | `#D0C4D8` | 12.57:1 | triton (6.16) |
| Dysnomia | Moon | `#A3ABB5` | 9.05:1 | rhea (4.33) |
| Hiʻiaka | Moon | `#D8F4F5` | 18.18:1 | haumea (5.04) |
| Namaka | Moon | `#A9D3CF` | 12.91:1 | ariel (9.46) |
| 2 Pallas | Asteroid | `#9FA0B8` | 8.19:1 | psyche (8.64) |
| 3 Juno | Asteroid | `#A77C5A` | 5.67:1 | hyperion (5.44) |
| 4 Vesta | Asteroid | `#D8C9B0` | 12.90:1 | moon (9.33) |
| 10 Hygiea | Asteroid | `#5E6647` | 3.47:1 | iapetus (14.70) |
| 16 Psyche | Asteroid | `#8494A6` | 6.77:1 | nereid (5.34) |
| 433 Eros | Asteroid | `#B58A61` | 6.77:1 | juno (6.23) |
| 101955 Bennu | Asteroid | `#53575B` | 2.88:1 | phoebe (8.19) |
| 99942 Apophis | Asteroid | `#7D7068` | 4.39:1 | callisto (5.66) |
| 1P/Halley | Comet | `#78D9F2` | 13.01:1 | encke (8.73) |
| 2P/Encke | Comet | `#5FC4D6` | 10.35:1 | uranus (7.82) |
| 9P/Tempel 1 | Comet | `#91D3DD` | 12.57:1 | uranus (8.53) |
| 67P/Churyumov-Gerasimenko | Comet | `#70C9B6` | 10.72:1 | hartley_2 (13.47) |
| 103P/Hartley 2 | Comet | `#59E0C5` | 12.89:1 | churyumov_gerasimenko (13.47) |
| Hale-Bopp | Comet | `#8FAEFF` | 9.67:1 | earth (23.60) |
| NEOWISE | Comet | `#B5DDF4` | 14.61:1 | nix (9.94) |
| 3I/ATLAS | Comet | `#74A6C9` | 8.04:1 | nereid (13.98) |
