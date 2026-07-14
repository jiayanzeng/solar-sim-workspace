# Bright Star Catalog starfield provenance

- Dataset: Bright Star Catalog (`BSC5P`)
- Publisher: NASA High Energy Astrophysics Science Archive Research Center
  (HEASARC)
- Dataset identifier: `ivo://nasa.heasarc/bsc5p`
- NASA Open Data record:
  https://data.nasa.gov/dataset/bright-star-catalog
- HEASARC catalog documentation:
  https://heasarc.gsfc.nasa.gov/W3Browse/all/bsc5p.html
- Original catalog reference: Hoffleit, D. and Warren, Jr., W.H. (1991),
  *The Bright Star Catalog, 5th Revised Edition (Preliminary Version)*,
  VizieR V/50
- License: the NASA Open Data record marks access as public and identifies
  https://www.usa.gov/government-works as the dataset license.
- Retrieved: 2026-07-14
- TAP endpoint: https://heasarc.gsfc.nasa.gov/xamin/vo/tap/sync
- ADQL query:
  `SELECT hr,ra,dec,vmag FROM bsc5p ORDER BY hr`
- Source response format: VOTable 1.4 BINARY, base64 encoded
- Source response SHA-256:
  `e4f539290c7f6303695f6fafb07618d66a23ce55e5898cb1145769aaa0913b6f`
- Source composition: 9,110 HR rows. The baker excludes the 14 historical
  non-stellar rows explicitly identified by HEASARC, which retain coordinates
  but have null visual magnitudes, leaving 9,096 stars.
- Bake command:
  `cargo run -p xtask -- bake-starfield --source PATH_TO_BSC5P_VOTABLE --out assets/starfield.bsc`
- Bake transform: sort by visual magnitude then HR number; retain the brightest
  5,000; rotate J2000 equatorial unit vectors into ecliptic-J2000 using the
  23.4392911-degree mean obliquity; encode magnitude-scaled point sizes.
- Vendored derived file: `starfield.bsc`
- Derived file SHA-256:
  `312d6b4a94f0fd62e4877f7c63d36ba8af7ac084537f05d07faead3ef6fd628b`
- Use: retained celestial-sphere starfield for `solar-sim`; this metadata is
  the WP17 licensing-audit input.

HEASARC states that BSC5P was created in 1995 from an ADC/CDS file and later
corrected by HEASARC, including positions for 14 historical non-stellar HR
objects and several 2014 position fixes. NASA, HEASARC, Hoffleit, Warren, and
VizieR are acknowledged as provenance; their names do not imply endorsement.
