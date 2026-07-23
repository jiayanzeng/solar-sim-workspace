# Dwarf surface appearance review — 2026-07-23

## Decision

This review implements Rev E Q25 policy 1. Ceres and Pluto use resolved global
mission mosaics; Charon was available in the same audited NASA pass and is
included as approved. Eris, Haumea, Makemake, Gonggong, Quaoar, Orcus, and
Sedna use one flat representative albedo each. A flat color communicates the
measured broad visible-color family without inventing geography that has not
been resolved.

The representative RGB values are renderer choices, not calibrated
reflectance spectra and not claims of human-eye “true color.” Textures and
colors affect only material appearance. Catalog radius, propagated position,
shape, rotation, picking, and orbital geometry remain unchanged.

## Resolved mission mosaics

| Body | Runtime asset | Source product | Review result |
|---|---|---|---|
| Ceres | `textures/ceres.ktx2` | USGS Astrogeology, *Ceres Dawn FC Global Mosaic 140m* browse image | Accepted. Resolved Dawn Framing Camera global mosaic; public-domain US Government work; exact source and output hashes in `ceres.license.json`. |
| Pluto | `textures/pluto.ktx2` | NASA 3D Resources, `Images and Textures/Pluto/Pluto.jpg` | Accepted. Resolved New Horizons global map; public-domain US Government work; exact source and output hashes in `pluto.license.json`. |
| Charon | `textures/charon.ktx2` | NASA 3D Resources, `Images and Textures/Pluto - Charon/Pluto - Charon.jpg` | Accepted in the same pass. Resolved New Horizons global map; public-domain US Government work; exact source and output hashes in `charon.license.json`. |

Primary source and license routes:

- [USGS Ceres Dawn global mosaic](https://astrogeology.usgs.gov/search/map/ceres_dawn_fc_global_mosaic_140m)
- [USGS copyrights and credits](https://www.usgs.gov/information-policies-and-instructions/copyrights-and-credits)
- [NASA 3D Resources](https://github.com/nasa/NASA-3D-Resources)

All three source images are equirectangular 2:1 maps. The reviewed conversion
is RGB Lanczos resize to 2048×1024 PPM followed by the repository's
dependency-free `xtask convert-texture` KTX2 encoder. The sidecar audit
verifies source URL, source SHA-256, dimensions, license route, transform,
output dimensions, and shipped-asset SHA-256.

## Representative albedo

| Body | sRGB | Broad measured appearance represented | Evidence route |
|---|---:|---|---|
| Eris | `#E8E5DF` | bright, nearly neutral | Szakáts et al. report visible colors compatible with the lowest-redness TNOs and compare Eris with bright, nearly neutrally colored Haumea-family surfaces. |
| Haumea | `#DDECF0` | bright, nearly neutral water-ice family | Lacerda reports subtle color variation on an otherwise water-ice-dominated body; the flat value deliberately omits the unresolved darker/redder region. |
| Makemake | `#B86E52` | red optical slope | Lorenzi et al. measure Makemake's visible spectral slope; the renderer uses only a uniform reddish family, not inferred markings. |
| Gonggong | `#A64A48` | steep red spectral slope | Emery et al. find steep red slopes and irradiation products in JWST spectra of Gonggong, Quaoar, and Sedna. |
| Quaoar | `#A87363` | red, less saturated representative | Emery et al. find a steep red slope while the spectral inventory differs from Gonggong and Sedna; no spatial detail is inferred. |
| Orcus | `#8C9199` | comparatively neutral/gray, water-ice-bearing | de Bergh et al. report strong water-ice absorption and published photometry classifies Orcus among gray TNOs. |
| Sedna | `#8F3E43` | very red optical family | Emery et al. find a steep red slope; Barucci et al. report a largely featureless near-infrared spectrum at the available signal-to-noise. |

Scientific reference routes:

- Szakáts et al. (2023), [*Rotational Phase Dependent J−H Colour of the Dwarf Planet Eris*](https://doi.org/10.1088/1538-3873/ad0b31)
- Lacerda (2009), [*Time-Resolved Near-Infrared Photometry of Extreme Kuiper Belt Object Haumea*](https://arxiv.org/abs/0811.3732)
- Lorenzi et al. (2020), [*The dwarf planet Makemake as seen by X-Shooter*](https://doi.org/10.1093/mnras/staa2264)
- Emery et al. (2024), [*A Tale of 3 Dwarf Planets: Ices and Organics on Sedna, Gonggong, and Quaoar from JWST Spectroscopy*](https://arxiv.org/abs/2309.15230)
- de Bergh et al. (2005), [*Near-Infrared Surface Properties of Sedna and Orcus*](https://authors.library.caltech.edu/records/wg3zy-nav52/latest)

## Audit conclusion

The product may call the Ceres, Pluto, and Charon assets resolved mission
maps. It must call the remaining seven values representative albedo (or
representative color), never actual photographs, resolved maps, or known
surface markings. Missing asset loads fall back to the catalog color without
changing simulation truth.
