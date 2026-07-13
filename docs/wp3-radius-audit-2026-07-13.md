# WP3 radius audit — 2026-07-13

This is the human-review sheet for the curated `radius_km` values in
`xtask/src/manifest.rs`. It compares all 66 manifest entries with current
physical-size references and flags central-value differences greater than
2%. The human approved all eight flagged central-value changes on 2026-07-13;
the table preserves the pre-approval values as an audit trail.

## Sources and conventions

- **JPP:** [JPL Planetary Physical Parameters](https://ssd.jpl.nasa.gov/planets/phys_par.html), using volume-equivalent mean radius.
- **JSP:** [JPL Planetary Satellite Physical Parameters](https://ssd.jpl.nasa.gov/sats/phys_par/), using mean radius.
- **SBDB:** live [JPL SBDB physical parameters](https://ssd-api.jpl.nasa.gov/doc/sbdb.html) queried with `phys-par=true`; reported effective diameter is divided by two.
- **LIT:** primary literature linked in the notes below where JPL has no current physical radius or a newer measurement supersedes its summary table.
- Differences are measured against the manifest value. A flag can still be statistically consistent when the published uncertainty is broad; the threshold is a review trigger, not a claim that the old value is physically impossible.

## Eight approved changes

| id | Pre-approval radius (km) | Approved radius (km) | Difference | Source / decision |
|---|---:|---:|---:|---|
| `himalia` | 75.0 | **85.0** | +13.3% | JSP central value 85 ± 10 km; approved. |
| `nix` | 19.5 | **18.0** | −7.7% | JSP / Stern et al. (2018), 18.0 ± 1.0 km; approved. |
| `hydra` | 18.0 | **18.5** | +2.8% | JSP / Stern et al. (2018), 18.5 ± 1.0 km; approved. |
| `hiiaka` | 160.0 | **185.0** | +15.6% | [Fernández-Valenzuela et al. (2025)](https://www.nature.com/articles/s41467-025-65749-1), volume-equivalent diameter 370 ± 20 km; approved. |
| `namaka` | 85.0 | **75.0** | −11.8% | [Müller et al. (2019)](https://arxiv.org/abs/1811.09476), thermal diameter about 150 ± 50 km; approved with uncertainty retained in provenance. |
| `hygiea` | 217.0 | **203.56** | −6.2% | SBDB diameter 407.12 ± 6.8 km; approved. |
| `churyumov_gerasimenko` | 2.0 | **1.7** | −15.0% | SBDB diameter 3.4 ± 0.1 km, Sierks et al. (2015); approved. |
| `hartley_2` | 0.6 | **0.8** | +33.3% | SBDB diameter 1.6 km, Lamy et al. (2004); approved. |

## Remaining 58 values — within 2% or already human-approved

| id | Manifest → reference radius (km) | Source | Result |
|---|---:|---|---|
| `sun` | 695700 → 695700 | Horizons/JPP | keep |
| `mercury` | 2439.7 → 2439.4 | JPP | keep |
| `venus` | 6051.8 → 6051.8 | JPP | keep |
| `earth` | 6371.0 → 6371.0084 | JPP | keep |
| `mars` | 3389.5 → 3389.50 | JPP | keep |
| `jupiter` | 69911 → 69911 | JPP | keep |
| `saturn` | 58232 → 58232 | JPP | keep |
| `uranus` | 25362 → 25362 | JPP | keep |
| `neptune` | 24622 → 24622 | JPP | keep |
| `moon` | 1737.4 → 1737.4 | JSP | keep |
| `phobos` | 11.1 → 11.08 | JSP | keep |
| `deimos` | 6.2 → 6.2 | JSP | keep |
| `io` | 1821.6 → 1821.49 | JSP | keep |
| `europa` | 1560.8 → 1560.8 | JSP | keep |
| `ganymede` | 2634.1 → 2631.2 | JSP | keep |
| `callisto` | 2410.3 → 2410.3 | JSP | keep |
| `amalthea` | 83.5 → 83.5 | JSP | keep |
| `mimas` | 198.2 → 198.2 | JSP | keep |
| `enceladus` | 252.1 → 252.1 | JSP | keep |
| `tethys` | 531.1 → 531.1 | JSP | keep |
| `dione` | 561.4 → 561.4 | JSP | keep |
| `rhea` | 763.8 → 763.5 | JSP | keep |
| `titan` | 2574.7 → 2574.76 | JSP | keep |
| `hyperion` | 135.0 → 135.0 | JSP | keep |
| `iapetus` | 734.5 → 734.3 | JSP | keep |
| `phoebe` | 106.5 → 106.5 | JSP | keep |
| `miranda` | 235.8 → 235.8 | JSP | keep |
| `ariel` | 578.9 → 578.9 | JSP | keep |
| `umbriel` | 584.7 → 584.7 | JSP | keep |
| `titania` | 788.4 → 788.9 | JSP | keep |
| `oberon` | 761.4 → 761.4 | JSP | keep |
| `triton` | 1353.4 → 1352.6 | JSP | keep |
| `nereid` | 170 → 170 ± 25 | JSP | keep |
| `proteus` | 210 → 208 ± 8 | JSP | keep |
| `ceres` | 469.7 → 469.7 | JPP/SBDB | keep |
| `pluto` | 1188.3 → 1188.3 | JPP | keep |
| `eris` | 1163 → 1163 ± 6 | [Sicardy et al. (2011)](https://www.nature.com/articles/nature10550) | keep |
| `haumea` | 780 → 772 +20/−19 | [Proudfoot et al. (2026)](https://arxiv.org/abs/2605.28636) | keep; within current uncertainty |
| `makemake` | 715 → about 714 | JPP / Brown (2013) | keep |
| `gonggong` | 615 → 615 ± 25 | [Kiss et al. (2019)](https://arxiv.org/abs/1903.05439) | keep |
| `quaoar` | 545 → 547.2 ± 2.3 | [Margoti et al. (2026)](https://arxiv.org/abs/2607.06450) | keep |
| `orcus` | 458 → 458.5 ± 12.5 | [Fornasier et al. (2013)](https://arxiv.org/abs/1305.0449) | keep |
| `sedna` | 500 → 497.5 ± 40 | [Pál et al. (2012)](https://doi.org/10.1051/0004-6361/201218874) | keep |
| `charon` | 606 → 606.0 ± 0.5 | JSP | keep |
| `dysnomia` | 350 → 350 ± 57.5 | [Brown & Butler (2018)](https://arxiv.org/abs/1801.07221) | keep |
| `pallas` | 256 → 256.5 ± 3 | SBDB | keep |
| `juno` | 123 → 123.298 ± 5.297 | SBDB | keep |
| `vesta` | 262.7 → 261.385 ± 0.05 | SBDB | keep |
| `psyche` | 113 → 111 +2/−0.5 | SBDB | keep |
| `eros` | 8.4 → 8.42 ± 0.03 | SBDB | keep |
| `bennu` | 0.245 → 0.24222 ± 0.00015 | SBDB | keep |
| `apophis` | 0.17 → 0.17 ± 0.02 | SBDB | keep |
| `halley` | 5.5 → 5.5 | SBDB | keep |
| `encke` | 2.4 → 2.4 | SBDB | keep |
| `tempel_1` | 3.0 → 3.0 ± 0.1 | SBDB | keep |
| `hale_bopp` | 30 → 30 ± 10 | SBDB | keep |
| `neowise` | 2.5 → about 2.5; upper limit 4.72 | [Biver et al. (2022)](https://doi.org/10.1051/0004-6361/202244970) | keep |
| `3i_atlas` | 0.5 → adopted 0.5 | Human Q3 decision; HST/NGA bracket in source | keep |

## Sign-off record

Human approval on 2026-07-13 accepted the eight central-value changes above
and the other 58 values as reviewed. The manifest changes are limited to those
eight radii, with per-body physical provenance; the generated catalog is
regenerated through `xtask` and measured evidence is recorded in `TASKS.md`.
