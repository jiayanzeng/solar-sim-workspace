//! WP3/WP10 curated catalog authoring — Rev C §§5.2 and 9.2.
//!
//! Split of responsibility, deliberately:
//! - **Curated here, human-reviewed:** identity (id/name/designation/aliases),
//!   taxonomy (category/parent), physical radius, GM for parents, display
//!   color, Major/All moon visibility, description blurbs, and the source route.
//! - **Generated from JPL, never hand-typed:** every orbital element, epoch,
//!   secular rate, and mean motion.
//!
//! REVIEW STATUS: all 66 radii and every curated parent GM were human-reviewed
//! on 2026-07-13. All 66 descriptions and public content sources were reviewed
//! on 2026-07-22. See the audit evidence in `TASKS.md`.

use sim_core::catalog::Category;

/// JPL DE440 gravitational parameters, km³/s², human-reviewed 2026-07-13.
/// Mars through Neptune are the DE440 system values; see the GM audit.
pub const GM_SUN: f64 = 132_712_440_041.279_42;
pub const GM_MERCURY: f64 = 22_031.868_551;
pub const GM_VENUS: f64 = 324_858.592_000;
pub const GM_EARTH: f64 = 398_600.435_507;
pub const GM_MARS: f64 = 42_828.375_816;
pub const GM_JUPITER: f64 = 126_712_764.100_000;
pub const GM_SATURN: f64 = 37_940_584.841_800;
pub const GM_URANUS: f64 = 5_794_556.400_000;
pub const GM_NEPTUNE: f64 = 6_836_527.100_580;
// TNO parent-system values (needed because they carry moons), human-reviewed
// 2026-07-13. Pluto deliberately includes Charon for the best two-body fit to
// every Pluto-system moon: 869.6 + 105.9 = 975.5 km³/s².
pub const GM_PLUTO: f64 = 9.755e2;
pub const GM_ERIS: f64 = 1.108e3;
pub const GM_HAUMEA: f64 = 2.67e2;

// Category color LUT (per Rev B §9: planets individually colored; other
// categories share a hue). Our palette, not NASA's.
const C_SUN: (u8, u8, u8) = (255, 214, 140);
const C_MERCURY: (u8, u8, u8) = (158, 158, 158);
const C_VENUS: (u8, u8, u8) = (222, 184, 135);
const C_EARTH: (u8, u8, u8) = (86, 141, 235);
const C_MARS: (u8, u8, u8) = (204, 101, 66);
const C_JUPITER: (u8, u8, u8) = (211, 177, 140);
const C_SATURN: (u8, u8, u8) = (226, 205, 159);
const C_URANUS: (u8, u8, u8) = (148, 207, 216);
const C_NEPTUNE: (u8, u8, u8) = (99, 125, 222);
const C_DWARF: (u8, u8, u8) = (186, 156, 255);
const C_AST: (u8, u8, u8) = (158, 163, 170);
const C_COMET: (u8, u8, u8) = (166, 216, 232);
const C_MOON: (u8, u8, u8) = (198, 189, 175);

/// WP15 texture assignments are curated identity data, never an emitter
/// post-process. Paths are relative to Bevy's asset root and therefore flow
/// through the same manifest -> generated catalog route as every other
/// display field.
pub fn texture_path(id: &str) -> Option<&'static str> {
    Some(match id {
        "sun" => "textures/sun.ktx2",
        "mercury" => "textures/mercury.ktx2",
        "venus" => "textures/venus.ktx2",
        "earth" => "textures/earth.ktx2",
        "mars" => "textures/mars.ktx2",
        "jupiter" => "textures/jupiter.ktx2",
        "saturn" => "textures/saturn.ktx2",
        "uranus" => "textures/uranus.ktx2",
        "neptune" => "textures/neptune.ktx2",
        // The highest-value public-domain major-moon maps available in NASA's
        // 3D Resources collection. Other bodies retain their catalog colors.
        "moon" => "textures/moon.ktx2",
        "io" => "textures/io.ktx2",
        "europa" => "textures/europa.ktx2",
        "ganymede" => "textures/ganymede.ktx2",
        "callisto" => "textures/callisto.ktx2",
        "titan" => "textures/titan.ktx2",
        _ => return None,
    })
}

/// How the generator obtains orbital elements for a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// No orbit; physical constants only (the Sun).
    SunFixed,
    /// JPL Horizons ELEMENTS, heliocentric (`CENTER='500@10'`), sampled at the
    /// catalog epoch, epoch+1d (mean-motion fit) and 1800–2300 coarse epochs
    /// (secular fit).
    HorizonsPlanet { command: &'static str },
    /// JPL Horizons ELEMENTS, parent-centric (`CENTER='500@<parent>'`),
    /// single sample at the catalog epoch.
    HorizonsMoon {
        command: &'static str,
        center: &'static str,
    },
    /// Same as `HorizonsMoon`, but the numeric COMMAND (and possibly the
    /// center designator) must first be resolved through the Horizons lookup
    /// API — the TNO satellites have no stable well-known codes.
    /// See spec "Open items". In `--fixtures` mode a plain `<id>.json`
    /// Horizons response is accepted directly.
    HorizonsLookupMoon {
        sstr: &'static str,
        parent_sstr: &'static str,
    },
    /// JPL Small-Body Database (`sbdb.api?sstr=...&full-prec=true`),
    /// heliocentric ecliptic-J2000 elements at the SBDB epoch. Comets without
    /// a mean anomaly are re-based to perihelion (epoch := Tp, M0 := 0).
    Sbdb { sstr: &'static str },
}

pub struct Entry {
    pub id: &'static str,
    pub name: &'static str,
    pub designation: Option<&'static str>,
    pub aliases: &'static [&'static str],
    pub category: Category,
    pub parent: Option<&'static str>,
    /// Curated WP10 display tier; assigned from `MAJOR_MOON_IDS` below.
    pub is_major_moon: bool,
    pub gm_km3_s2: Option<f64>,
    /// Human-reviewed mean/effective radius; see the 2026-07-13 WP3 audit.
    pub radius_km: f64,
    pub color: (u8, u8, u8),
    pub route: Route,
    /// Curated two-to-four-sentence WP10 description shown in the Info tab.
    pub blurb: &'static str,
    /// Provenance note carried into the emitted `source` field.
    pub source_note: &'static str,
}

const PHYS_NOTE: &str =
    "phys: curated radius reviewed 2026-07-13 (docs/wp3-radius-audit-2026-07-13.md)";

/// Human-authored WP10 copy and the public source used to review it.
///
/// Keep the source beside the text: the generated catalog carries it in each
/// body's `source` field, so a description can never ship without its audit
/// trail. Orbital and physical claims additionally retain the JPL references
/// assembled by [`source_string`].
fn curated_description(id: &str) -> (&'static str, &'static str) {
    match id {
        "sun" => (
            "The Sun is the star at the center of the solar system, and its gravity holds the system together. Its light and heat power most surface life on Earth and shape the space environment around every cataloged world.",
            "https://science.nasa.gov/sun/facts/",
        ),
        "mercury" => (
            "Mercury is the smallest planet and the closest planet to the Sun. Its cratered surface experiences extreme temperature changes while the planet completes an orbit in only 88 Earth days.",
            "https://science.nasa.gov/mercury/facts/",
        ),
        "venus" => (
            "Venus is close to Earth in size but is wrapped in a dense carbon-dioxide atmosphere. A runaway greenhouse effect makes its surface hotter than Mercury's, while thick clouds hide the ground from visible light.",
            "https://science.nasa.gov/venus/facts/",
        ),
        "earth" => (
            "Earth is a rocky planet with liquid surface oceans and an atmosphere rich in nitrogen and oxygen. It is the only world currently known to support life, and its active geology continually reshapes the surface.",
            "https://science.nasa.gov/earth/facts/",
        ),
        "mars" => (
            "Mars is a cold desert planet whose iron-bearing surface gives it a reddish appearance. It preserves giant volcanoes, deep canyons, polar ice, and evidence that liquid water flowed across its surface in the past.",
            "https://science.nasa.gov/mars/facts/",
        ),
        "jupiter" => (
            "Jupiter is the largest planet and a gas giant made mostly of hydrogen and helium. Its banded atmosphere hosts long-lived storms, while a powerful magnetic field governs an extensive system of rings and moons.",
            "https://science.nasa.gov/jupiter/facts/",
        ),
        "saturn" => (
            "Saturn is a hydrogen-and-helium gas giant with the solar system's most conspicuous ring system. The rings are made of countless pieces of ice and rock, and the planet is accompanied by a diverse family of moons.",
            "https://science.nasa.gov/saturn/facts/",
        ),
        "uranus" => (
            "Uranus is an ice giant whose rotation axis lies nearly in the plane of its orbit. This unusual tilt produces extreme seasons as the pale blue-green planet makes its long journey around the Sun.",
            "https://science.nasa.gov/uranus/facts/",
        ),
        "neptune" => (
            "Neptune is the outermost planet and a cold, blue ice giant. Its atmosphere contains rapidly moving clouds and some of the fastest measured winds in the solar system.",
            "https://science.nasa.gov/neptune/facts/",
        ),
        "moon" => (
            "Earth's Moon is a rocky, airless world whose surface records billions of years of impacts. It stabilizes Earth's axial wobble, drives most ocean tides, and remains the only world beyond Earth visited by humans.",
            "https://science.nasa.gov/moon/facts/",
        ),
        "phobos" => (
            "Phobos is the larger and innermost of Mars's two small moons. It is an irregular, heavily cratered body that circles Mars faster than the planet rotates, so it appears to cross the Martian sky from west to east.",
            "https://science.nasa.gov/mars/moons/phobos/",
        ),
        "deimos" => (
            "Deimos is the smaller and more distant of Mars's two moons. Its irregular shape is softened in images by a thick layer of loose surface material that partly fills many craters.",
            "https://science.nasa.gov/mars/moons/deimos/",
        ),
        "io" => (
            "Io is the innermost of Jupiter's four large Galilean moons. Tidal heating drives widespread volcanism, continually renewing a colorful surface marked by lava flows and sulfur compounds.",
            "https://science.nasa.gov/jupiter/moons/io/",
        ),
        "europa" => (
            "Europa is an ice-covered Galilean moon with a young surface crossed by dark ridges and bands. Multiple lines of evidence indicate a salty liquid-water ocean beneath its shell, making it a major target for astrobiology.",
            "https://science.nasa.gov/jupiter/moons/europa/",
        ),
        "ganymede" => (
            "Ganymede is Jupiter's largest moon and the largest moon in the solar system. It is the only moon known to generate its own magnetic field, and evidence points to a deep ocean beneath its icy crust.",
            "https://science.nasa.gov/jupiter/moons/ganymede/",
        ),
        "callisto" => (
            "Callisto is the outermost of Jupiter's four Galilean moons and has an ancient, densely cratered surface. Its interior is less differentiated than Ganymede's, although magnetic measurements suggest a subsurface salty ocean.",
            "https://science.nasa.gov/jupiter/moons/callisto/",
        ),
        "amalthea" => (
            "Amalthea is a small, irregular moon orbiting inside the paths of Jupiter's Galilean moons. It is unusually red, heavily cratered, and contributes material to Jupiter's faint gossamer ring system.",
            "https://science.nasa.gov/jupiter/jupiter-moons/amalthea/",
        ),
        "himalia" => (
            "Himalia is the largest member of a distant group of irregular moons orbiting Jupiter. Its inclined orbit and the similar paths of nearby moons are consistent with fragments of a captured parent body.",
            "https://science.nasa.gov/jupiter/jupiter-moons/",
        ),
        "mimas" => (
            "Mimas is a small icy moon of Saturn dominated visually by the large Herschel impact crater. Despite its battered surface, measurements of its motion provide evidence for an ocean beneath the outer ice.",
            "https://science.nasa.gov/saturn/moons/mimas/",
        ),
        "enceladus" => (
            "Enceladus is a small icy moon with a global ocean beneath its crust. Jets near its south pole spray water vapor and ice into space, supplying material to Saturn's E ring and allowing spacecraft to sample the ocean indirectly.",
            "https://science.nasa.gov/saturn/moons/enceladus/",
        ),
        "tethys" => (
            "Tethys is an icy Saturnian moon with a low density and a bright, heavily cratered surface. The vast Odysseus crater and the long Ithaca Chasma canyon record major events in its geological history.",
            "https://science.nasa.gov/saturn/moons/tethys/",
        ),
        "dione" => (
            "Dione is an icy moon of Saturn with a mixture of old cratered terrain and brighter tectonic fractures. Cassini observations also detected a tenuous oxygen-bearing exosphere around it.",
            "https://science.nasa.gov/saturn/moons/dione/",
        ),
        "rhea" => (
            "Rhea is Saturn's second-largest moon and is composed largely of water ice. Its old, cratered surface is crossed by bright fractures, and a very thin atmosphere contains oxygen and carbon dioxide.",
            "https://science.nasa.gov/saturn/moons/rhea/",
        ),
        "titan" => (
            "Titan is Saturn's largest moon and the only moon with a dense atmosphere. Methane and ethane form clouds, rain, rivers, lakes, and seas on its surface, while a water-rich ocean is thought to lie below the crust.",
            "https://science.nasa.gov/saturn/moons/titan/",
        ),
        "hyperion" => (
            "Hyperion is an irregular, porous moon of Saturn with a deeply pitted, sponge-like surface. Its elongated shape and gravitational interactions with Titan contribute to a chaotic, unpredictable rotation.",
            "https://science.nasa.gov/saturn/moons/hyperion/",
        ),
        "iapetus" => (
            "Iapetus is a large icy moon with one hemisphere much darker than the other. A prominent equatorial ridge gives the body a distinctive profile, while its distant orbit offers broad views of Saturn's system.",
            "https://science.nasa.gov/saturn/moons/iapetus/",
        ),
        "phoebe" => (
            "Phoebe is a dark, irregular outer moon moving around Saturn on a retrograde orbit. Its distant, inclined path and surface composition support the interpretation that it was captured rather than formed with Saturn's regular moons.",
            "https://science.nasa.gov/saturn/moons/phoebe/",
        ),
        "miranda" => (
            "Miranda is the smallest and innermost of Uranus's five major moons. Its patchwork surface combines old cratered terrain with enormous fault canyons and younger-looking regions shaped by past geological activity.",
            "https://science.nasa.gov/uranus/moons/miranda/",
        ),
        "ariel" => (
            "Ariel is one of Uranus's five major moons and has the brightest surface among them. Fault valleys and comparatively sparse large craters indicate that geological activity resurfaced substantial areas in the past.",
            "https://science.nasa.gov/uranus/moons/ariel/",
        ),
        "umbriel" => (
            "Umbriel is the darkest of Uranus's five major moons. Its ancient, heavily cratered surface includes a conspicuous bright ring associated with the crater Wunda.",
            "https://science.nasa.gov/uranus/moons/umbriel/",
        ),
        "titania" => (
            "Titania is the largest moon of Uranus. Its icy surface is cut by large canyons and fault scarps, evidence that the interior expanded and fractured the crust during its history.",
            "https://science.nasa.gov/uranus/moons/titania/",
        ),
        "oberon" => (
            "Oberon is the outermost of Uranus's five major moons and the second largest. Its old surface is heavily cratered, with dark material visible on the floors of several craters.",
            "https://science.nasa.gov/uranus/moons/oberon/",
        ),
        "triton" => (
            "Triton is Neptune's largest moon and follows a retrograde orbit opposite the planet's rotation. Voyager 2 observed a young icy surface and active nitrogen geysers, while its orbit and Pluto-like properties point to capture from the Kuiper Belt.",
            "https://science.nasa.gov/neptune/moons/triton/",
        ),
        "nereid" => (
            "Nereid is a distant Neptunian moon with one of the most eccentric satellite orbits known. That unusual path may reflect capture or a major disturbance when Neptune acquired Triton.",
            "https://science.nasa.gov/neptune/moons/nereid/",
        ),
        "proteus" => (
            "Proteus is one of Neptune's largest moons but remained undiscovered until Voyager 2 imaged it in 1989. It is dark, heavily cratered, and irregularly shaped despite being close to the size at which gravity rounds many worlds.",
            "https://science.nasa.gov/neptune/moons/proteus/",
        ),
        "ceres" => (
            "Ceres is the largest object in the main asteroid belt and the only dwarf planet in the inner solar system. Dawn found a mixture of rock and ice, widespread evidence of past water activity, and bright salt deposits in Occator Crater.",
            "https://science.nasa.gov/dwarf-planets/ceres/facts/",
        ),
        "pluto" => (
            "Pluto is a dwarf planet in the Kuiper Belt with five known moons. New Horizons revealed a varied world of nitrogen-ice plains, mountains of water ice, layered haze, and ongoing surface change.",
            "https://science.nasa.gov/dwarf-planets/pluto/facts/",
        ),
        "eris" => (
            "Eris is a distant dwarf planet in the scattered disc and is close to Pluto in size. Its discovery intensified debate over the meaning of planet and contributed to the adoption of the dwarf-planet category.",
            "https://science.nasa.gov/dwarf-planets/eris/",
        ),
        "haumea" => (
            "Haumea is a rapidly rotating dwarf planet with an elongated shape, two moons, and a ring. Its water-ice-rich surface and associated family of small bodies point to a major collision early in its history.",
            "https://science.nasa.gov/dwarf-planets/haumea/",
        ),
        "makemake" => (
            "Makemake is a bright, methane-bearing dwarf planet in the Kuiper Belt. It is smaller than Pluto, has one known moon, and takes more than three centuries to complete an orbit around the Sun.",
            "https://science.nasa.gov/dwarf-planets/makemake/",
        ),
        "gonggong" => (
            "Gonggong is a large trans-Neptunian object on an elongated and inclined orbit beyond Neptune. It takes several centuries to circle the Sun and travels much farther away at aphelion than it comes at perihelion.",
            "https://ssd.jpl.nasa.gov/tools/sbdb_lookup.html#/?sstr=Gonggong",
        ),
        "quaoar" => (
            "Quaoar is a large Kuiper Belt object following a less elongated orbit than many distant catalog worlds. Its heliocentric journey still takes nearly three centuries and remains beyond Neptune throughout the orbit.",
            "https://ssd.jpl.nasa.gov/tools/sbdb_lookup.html#/?sstr=Quaoar",
        ),
        "orcus" => (
            "Orcus is a large Kuiper Belt object trapped in a two-to-three orbital resonance with Neptune. Its orbital period is close to Pluto's, but the two objects occupy different parts of their similarly shaped paths.",
            "https://ssd.jpl.nasa.gov/tools/sbdb_lookup.html#/?sstr=Orcus",
        ),
        "sedna" => (
            "Sedna follows an extremely elongated orbit that carries it far beyond the Kuiper Belt. Even near perihelion it remains distant from the Sun, making its orbit an important clue to the structure and early history of the outer solar system.",
            "https://ssd.jpl.nasa.gov/tools/sbdb_lookup.html#/?sstr=Sedna",
        ),
        "charon" => (
            "Charon is Pluto's largest moon and is about half Pluto's diameter. The two bodies are mutually tidally locked and orbit a common center of mass outside Pluto, giving the pair an unusually balanced relationship.",
            "https://science.nasa.gov/dwarf-planets/pluto/moons/charon/",
        ),
        "nix" => (
            "Nix is one of Pluto's four small outer moons and follows a nearly circular path around the Pluto-Charon pair. New Horizons found an elongated body with a bright surface and a conspicuous reddish impact crater.",
            "https://science.nasa.gov/dwarf-planets/pluto/moons/nix/",
        ),
        "hydra" => (
            "Hydra is the outermost of Pluto's known moons and circles the Pluto-Charon pair. New Horizons images show an irregular, highly reflective body whose water-ice surface includes two joined-looking lobes.",
            "https://science.nasa.gov/dwarf-planets/pluto/moons/hydra/",
        ),
        "dysnomia" => (
            "Dysnomia is the only known moon of the dwarf planet Eris. Tracking its orbit allowed astronomers to determine the mass of the Eris system, while the moon itself remains only sparsely resolved.",
            "https://science.nasa.gov/dwarf-planets/eris/",
        ),
        "hiiaka" => (
            "Hiʻiaka is the larger and outer moon of the dwarf planet Haumea. Its bright water-ice surface links it compositionally with Haumea and other members of the same collisional family.",
            "https://science.nasa.gov/dwarf-planets/haumea/",
        ),
        "namaka" => (
            "Namaka is the smaller and inner known moon of Haumea. Its orbit is dynamically influenced by Hiʻiaka, helping astronomers investigate the masses and shapes within the Haumea system.",
            "https://science.nasa.gov/dwarf-planets/haumea/",
        ),
        "pallas" => (
            "2 Pallas is one of the largest objects in the main asteroid belt. Its cataloged path is substantially inclined to the ecliptic, distinguishing its orbit from the flatter paths of many main-belt objects.",
            "https://ssd.jpl.nasa.gov/tools/sbdb_lookup.html#/?sstr=2%20Pallas",
        ),
        "juno" => (
            "3 Juno is a large asteroid orbiting within the main belt between Mars and Jupiter. Its heliocentric path is both more eccentric and more inclined than Earth's nearly circular orbit.",
            "https://ssd.jpl.nasa.gov/tools/sbdb_lookup.html#/?sstr=3%20Juno",
        ),
        "vesta" => (
            "4 Vesta is one of the largest bodies in the asteroid belt and has a differentiated rocky interior. Dawn mapped a giant south-polar impact basin whose ejecta supplied many meteorites later found on Earth.",
            "https://science.nasa.gov/solar-system/asteroids/4-vesta/",
        ),
        "hygiea" => (
            "10 Hygiea is one of the largest objects in the main asteroid belt. Its reviewed effective diameter is about 407 kilometers, while its cataloged orbit remains entirely between Mars and Jupiter.",
            "https://ssd.jpl.nasa.gov/tools/sbdb_lookup.html#/?sstr=10%20Hygiea",
        ),
        "psyche" => (
            "16 Psyche is a large main-belt asteroid containing a substantial mixture of metal and rock. Its composition may preserve evidence about collisions and the interiors of early planetesimals, which is why a dedicated spacecraft is traveling there.",
            "https://science.nasa.gov/solar-system/asteroids/16-psyche/",
        ),
        "eros" => (
            "433 Eros is an elongated near-Earth asteroid with a heavily cratered and boulder-strewn surface. NEAR Shoemaker became the first spacecraft to orbit an asteroid there and later made a controlled touchdown.",
            "https://science.nasa.gov/solar-system/asteroids/433-eros/",
        ),
        "bennu" => (
            "101955 Bennu is a carbon-rich near-Earth asteroid assembled as a loosely bound rubble pile. OSIRIS-REx mapped and sampled its unexpectedly boulder-covered surface, returning material to Earth in 2023.",
            "https://science.nasa.gov/solar-system/asteroids/101955-bennu/",
        ),
        "apophis" => (
            "99942 Apophis is a stony near-Earth asteroid whose orbit will carry it exceptionally close to Earth in 2029. The encounter is not an impact threat, but Earth's gravity will alter the asteroid's orbit and rotation enough to create a valuable natural experiment.",
            "https://science.nasa.gov/solar-system/asteroids/apophis/",
        ),
        "halley" => (
            "1P/Halley is a short-period comet that returns to the inner solar system roughly every 76 years. It is the source of the Eta Aquariid and Orionid meteor showers, and spacecraft imaged its dark nucleus during the 1986 passage.",
            "https://science.nasa.gov/solar-system/comets/1p-halley/",
        ),
        "encke" => (
            "2P/Encke has the shortest orbital period of any well-known periodic comet, returning about every 3.3 years. Dust released along its orbit contributes to the Taurid meteor complex.",
            "https://science.nasa.gov/solar-system/comets/2p-encke/",
        ),
        "tempel_1" => (
            "9P/Tempel 1 is a periodic Jupiter-family comet with a dark, irregular nucleus. NASA's Deep Impact mission struck it with a probe in 2005, and Stardust later revisited the comet to examine the altered surface.",
            "https://science.nasa.gov/solar-system/comets/9p-tempel-1/",
        ),
        "churyumov_gerasimenko" => (
            "67P/Churyumov-Gerasimenko is a Jupiter-family comet with a distinctive two-lobed nucleus. Rosetta orbited it through an active passage near the Sun and deployed the Philae lander, providing extended close-range observations of a comet.",
            "https://science.nasa.gov/solar-system/comets/67p-churyumov-gerasimenko/",
        ),
        "hartley_2" => (
            "103P/Hartley 2 is a small, highly active Jupiter-family comet with an elongated nucleus. The EPOXI flyby showed jets driven largely by carbon dioxide carrying icy particles away from the surface.",
            "https://science.nasa.gov/solar-system/comets/103p-hartley-hartley-2/",
        ),
        "hale_bopp" => (
            "C/1995 O1 Hale-Bopp is a long-period comet with a large nucleus. It remained bright to unaided observers for an unusually long interval during its 1997 passage and displayed prominent dust and ion tails.",
            "https://science.nasa.gov/solar-system/comets/c-1995-o1-hale-bopp/",
        ),
        "neowise" => (
            "C/2020 F3 NEOWISE is a long-period comet discovered by the NEOWISE space telescope in 2020. It survived a close solar passage and became a prominent northern-sky object with visible dust and ion tails.",
            "https://science.nasa.gov/mission/neowise/",
        ),
        "3i_atlas" => (
            "3I/ATLAS is the third confirmed interstellar object observed passing through the solar system. Its hyperbolic path is not bound to the Sun, and cometary activity revealed an icy body surrounded by a coma.",
            "https://science.nasa.gov/solar-system/comets/3i-atlas/",
        ),
        _ => panic!("catalog identity {id} lacks a reviewed WP10 description"),
    }
}

/// Project display classification approved under TASKS Q8 on 2026-07-13.
///
/// This is intentionally a reviewable identity list rather than a radius
/// cutoff: the principal companions of small systems remain useful in Major
/// mode, while the giant-planet systems shed their smaller/irregular entries.
const MAJOR_MOON_IDS: &[&str] = &[
    "moon",
    "phobos",
    "deimos",
    "io",
    "europa",
    "ganymede",
    "callisto",
    "mimas",
    "enceladus",
    "tethys",
    "dione",
    "rhea",
    "titan",
    "iapetus",
    "miranda",
    "ariel",
    "umbriel",
    "titania",
    "oberon",
    "triton",
    "charon",
    "dysnomia",
    "hiiaka",
    "namaka",
];

macro_rules! planet {
    ($id:literal, $name:literal, $cmd:literal, $gm:expr, $r:expr, $col:expr) => {
        Entry {
            id: $id,
            name: $name,
            designation: None,
            aliases: &[],
            category: Category::Planet,
            parent: Some("sun"),
            is_major_moon: false,
            gm_km3_s2: Some($gm),
            radius_km: $r,
            color: $col,
            route: Route::HorizonsPlanet { command: $cmd },
            blurb: curated_description($id).0,
            source_note: "orbit: JPL Horizons ELEMENTS heliocentric ECLIPJ2000 (+fitted secular); GM: JPL DE440 (Park et al. 2021)",
        }
    };
}

macro_rules! moon {
    ($id:literal, $name:literal, $cmd:literal, $center:literal, $parent:literal, $r:expr) => {
        Entry {
            id: $id,
            name: $name,
            designation: None,
            aliases: &[],
            category: Category::Moon,
            parent: Some($parent),
            is_major_moon: false,
            gm_km3_s2: None,
            radius_km: $r,
            color: C_MOON,
            route: Route::HorizonsMoon {
                command: $cmd,
                center: $center,
            },
            blurb: curated_description($id).0,
            source_note: "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000",
        }
    };
}

macro_rules! sbdb {
    ($id:literal, $name:literal, $des:expr, $aliases:expr, $cat:expr, $gm:expr, $r:expr, $col:expr, $sstr:literal) => {
        Entry {
            id: $id,
            name: $name,
            designation: $des,
            aliases: $aliases,
            category: $cat,
            parent: Some("sun"),
            is_major_moon: false,
            gm_km3_s2: $gm,
            radius_km: $r,
            color: $col,
            route: Route::Sbdb { sstr: $sstr },
            blurb: curated_description($id).0,
            source_note: "orbit: JPL SBDB heliocentric ECLIPJ2000",
        }
    };
}

/// The 66-body Rev B catalog, in emit order (parents before children).
pub fn entries() -> Vec<Entry> {
    use Category::*;
    let mut v: Vec<Entry> = Vec::with_capacity(66);

    // --- Star (1) ---
    v.push(Entry {
        id: "sun",
        name: "Sun",
        designation: None,
        aliases: &["Sol"],
        category: Star,
        parent: None,
        is_major_moon: false,
        gm_km3_s2: Some(GM_SUN),
        radius_km: 695_700.0,
        color: C_SUN,
        route: Route::SunFixed,
        blurb: curated_description("sun").0,
        source_note: "no orbit (heliocentric anchor); GM: JPL DE440 (Park et al. 2021)",
    });

    // --- Planets (8) ---
    v.push(planet!(
        "mercury", "Mercury", "199", GM_MERCURY, 2439.7, C_MERCURY
    ));
    v.push(planet!("venus", "Venus", "299", GM_VENUS, 6051.8, C_VENUS));
    v.push(planet!("earth", "Earth", "399", GM_EARTH, 6371.0, C_EARTH));
    v.push(planet!("mars", "Mars", "499", GM_MARS, 3389.5, C_MARS));
    v.push(planet!(
        "jupiter", "Jupiter", "5", GM_JUPITER, 69_911.0, C_JUPITER
    ));
    v.push(planet!(
        "saturn", "Saturn", "6", GM_SATURN, 58_232.0, C_SATURN
    ));
    v.push(planet!(
        "uranus", "Uranus", "7", GM_URANUS, 25_362.0, C_URANUS
    ));
    v.push(planet!(
        "neptune", "Neptune", "8", GM_NEPTUNE, 24_622.0, C_NEPTUNE
    ));

    // --- Moons (32) ---
    // Earth
    v.push({
        let mut m = moon!("moon", "Moon", "301", "500@399", "earth", 1737.4);
        m.aliases = &["Luna"];
        m
    });
    // Mars
    v.push(moon!("phobos", "Phobos", "401", "500@499", "mars", 11.1));
    v.push(moon!("deimos", "Deimos", "402", "500@499", "mars", 6.2));
    // Jupiter
    v.push(moon!("io", "Io", "501", "500@599", "jupiter", 1821.6));
    v.push(moon!(
        "europa", "Europa", "502", "500@599", "jupiter", 1560.8
    ));
    v.push(moon!(
        "ganymede", "Ganymede", "503", "500@599", "jupiter", 2634.1
    ));
    v.push(moon!(
        "callisto", "Callisto", "504", "500@599", "jupiter", 2410.3
    ));
    v.push(moon!(
        "amalthea", "Amalthea", "505", "500@599", "jupiter", 83.5
    ));
    v.push({
        let mut m = moon!("himalia", "Himalia", "506", "500@599", "jupiter", 85.0);
        m.source_note = "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000; radius: JPL Planetary Satellite Physical Parameters 85 +/- 10 km";
        m
    });
    // Saturn
    v.push(moon!("mimas", "Mimas", "601", "500@699", "saturn", 198.2));
    v.push(moon!(
        "enceladus",
        "Enceladus",
        "602",
        "500@699",
        "saturn",
        252.1
    ));
    v.push(moon!("tethys", "Tethys", "603", "500@699", "saturn", 531.1));
    v.push(moon!("dione", "Dione", "604", "500@699", "saturn", 561.4));
    v.push(moon!("rhea", "Rhea", "605", "500@699", "saturn", 763.8));
    v.push(moon!("titan", "Titan", "606", "500@699", "saturn", 2574.7));
    v.push(moon!(
        "hyperion", "Hyperion", "607", "500@699", "saturn", 135.0
    ));
    v.push(moon!(
        "iapetus", "Iapetus", "608", "500@699", "saturn", 734.5
    ));
    v.push(moon!("phoebe", "Phoebe", "609", "500@699", "saturn", 106.5));
    // Uranus
    v.push(moon!(
        "miranda", "Miranda", "705", "500@799", "uranus", 235.8
    ));
    v.push(moon!("ariel", "Ariel", "701", "500@799", "uranus", 578.9));
    v.push(moon!(
        "umbriel", "Umbriel", "702", "500@799", "uranus", 584.7
    ));
    v.push(moon!(
        "titania", "Titania", "703", "500@799", "uranus", 788.4
    ));
    v.push(moon!("oberon", "Oberon", "704", "500@799", "uranus", 761.4));
    // Neptune
    v.push(moon!(
        "triton", "Triton", "801", "500@899", "neptune", 1353.4
    ));
    v.push(moon!(
        "nereid", "Nereid", "802", "500@899", "neptune", 170.0
    ));
    v.push(moon!(
        "proteus", "Proteus", "808", "500@899", "neptune", 210.0
    ));

    // --- Dwarf planets (9) — before their moons; parents must precede children ---
    v.push(sbdb!(
        "ceres",
        "Ceres",
        None,
        &["1 Ceres"],
        DwarfPlanet,
        None,
        469.7,
        C_DWARF,
        "Ceres"
    ));
    v.push({
        let mut e = sbdb!(
            "pluto",
            "Pluto",
            None,
            &["134340"],
            DwarfPlanet,
            Some(GM_PLUTO),
            1188.3,
            C_DWARF,
            "134340"
        );
        e.source_note = "orbit: JPL SBDB heliocentric ECLIPJ2000; parent GM: Pluto+Charon system 975.5 km^3/s^2 = 869.6 + 105.9 (Brozović et al. 2015)";
        e
    });
    v.push(sbdb!(
        "eris",
        "Eris",
        None,
        &[],
        DwarfPlanet,
        Some(GM_ERIS),
        1163.0,
        C_DWARF,
        "Eris"
    ));
    v.push(sbdb!(
        "haumea",
        "Haumea",
        None,
        &[],
        DwarfPlanet,
        Some(GM_HAUMEA),
        780.0,
        C_DWARF,
        "Haumea"
    ));
    v.push(sbdb!(
        "makemake",
        "Makemake",
        None,
        &[],
        DwarfPlanet,
        None,
        715.0,
        C_DWARF,
        "Makemake"
    ));
    v.push(sbdb!(
        "gonggong",
        "Gonggong",
        None,
        &[],
        DwarfPlanet,
        None,
        615.0,
        C_DWARF,
        "Gonggong"
    ));
    v.push(sbdb!(
        "quaoar",
        "Quaoar",
        None,
        &[],
        DwarfPlanet,
        None,
        545.0,
        C_DWARF,
        "Quaoar"
    ));
    v.push(sbdb!(
        "orcus",
        "Orcus",
        None,
        &[],
        DwarfPlanet,
        None,
        458.0,
        C_DWARF,
        "Orcus"
    ));
    v.push(sbdb!(
        "sedna",
        "Sedna",
        None,
        &[],
        DwarfPlanet,
        None,
        500.0,
        C_DWARF,
        "Sedna"
    ));

    // --- TNO moons (belong to the 32-moon count) ---
    // Pluto
    v.push(moon!("charon", "Charon", "901", "500@999", "pluto", 606.0));
    v.push({
        let mut m = moon!("nix", "Nix", "902", "500@999", "pluto", 18.0);
        m.source_note = "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000; radius: JPL Planetary Satellite Physical Parameters 18.0 +/- 1.0 km (Stern et al. 2018)";
        m
    });
    v.push({
        let mut m = moon!("hydra", "Hydra", "903", "500@999", "pluto", 18.5);
        m.source_note = "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000; radius: JPL Planetary Satellite Physical Parameters 18.5 +/- 1.0 km (Stern et al. 2018)";
        m
    });
    // Eris / Haumea (Horizons ids resolved at generation time — see spec Open items)
    v.push(Entry {
        id: "dysnomia",
        name: "Dysnomia",
        designation: None,
        aliases: &[],
        category: Moon,
        parent: Some("eris"),
        is_major_moon: false,
        gm_km3_s2: None,
        radius_km: 350.0,
        color: C_MOON,
        route: Route::HorizonsLookupMoon {
            sstr: "Dysnomia",
            parent_sstr: "Eris",
        },
        blurb: curated_description("dysnomia").0,
        source_note: "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000 (id via lookup)",
    });
    v.push(Entry {
        id: "hiiaka",
        name: "Hi\u{02bb}iaka",
        designation: None,
        aliases: &["Hiiaka"],
        category: Moon,
        parent: Some("haumea"),
        is_major_moon: false,
        gm_km3_s2: None,
        radius_km: 185.0,
        color: C_MOON,
        route: Route::HorizonsLookupMoon {
            sstr: "Hiiaka",
            parent_sstr: "Haumea",
        },
        blurb: curated_description("hiiaka").0,
        source_note: "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000 (id via lookup); radius: volume-equivalent diameter 370 +/- 20 km / 2 (Fernandez-Valenzuela et al. 2025)",
    });
    v.push(Entry {
        id: "namaka",
        name: "Namaka",
        designation: None,
        aliases: &[],
        category: Moon,
        parent: Some("haumea"),
        is_major_moon: false,
        gm_km3_s2: None,
        radius_km: 75.0,
        color: C_MOON,
        route: Route::HorizonsLookupMoon {
            sstr: "Namaka",
            parent_sstr: "Haumea",
        },
        blurb: curated_description("namaka").0,
        source_note: "orbit: JPL Horizons ELEMENTS parent-centric ECLIPJ2000 (id via lookup); radius: adopted from thermal diameter about 150 +/- 50 km / 2 (Muller et al. 2019)",
    });

    // --- Asteroids (8) ---
    v.push(sbdb!(
        "pallas",
        "2 Pallas",
        None,
        &["Pallas"],
        Asteroid,
        None,
        256.0,
        C_AST,
        "2 Pallas"
    ));
    v.push(sbdb!(
        "juno",
        "3 Juno",
        None,
        &["Juno"],
        Asteroid,
        None,
        123.0,
        C_AST,
        "3 Juno"
    ));
    v.push(sbdb!(
        "vesta",
        "4 Vesta",
        None,
        &["Vesta"],
        Asteroid,
        None,
        262.7,
        C_AST,
        "4 Vesta"
    ));
    v.push({
        let mut e = sbdb!(
            "hygiea",
            "10 Hygiea",
            None,
            &["Hygiea"],
            Asteroid,
            None,
            203.56,
            C_AST,
            "10 Hygiea"
        );
        e.source_note =
            "orbit: JPL SBDB heliocentric ECLIPJ2000; radius: SBDB diameter 407.12 +/- 6.8 km / 2";
        e
    });
    v.push(sbdb!(
        "psyche",
        "16 Psyche",
        None,
        &["Psyche"],
        Asteroid,
        None,
        113.0,
        C_AST,
        "16 Psyche"
    ));
    v.push(sbdb!(
        "eros",
        "433 Eros",
        None,
        &["Eros"],
        Asteroid,
        None,
        8.4,
        C_AST,
        "433 Eros"
    ));
    v.push(sbdb!(
        "bennu",
        "101955 Bennu",
        None,
        &["Bennu"],
        Asteroid,
        None,
        0.245,
        C_AST,
        "101955 Bennu"
    ));
    v.push(sbdb!(
        "apophis",
        "99942 Apophis",
        None,
        &["Apophis"],
        Asteroid,
        None,
        0.17,
        C_AST,
        "99942 Apophis"
    ));

    // --- Comets (8) ---
    v.push(sbdb!(
        "halley",
        "1P/Halley",
        Some("1P"),
        &["Halley"],
        Comet,
        None,
        5.5,
        C_COMET,
        "1P"
    ));
    v.push(sbdb!(
        "encke",
        "2P/Encke",
        Some("2P"),
        &["Encke"],
        Comet,
        None,
        2.4,
        C_COMET,
        "2P"
    ));
    v.push(sbdb!(
        "tempel_1",
        "9P/Tempel 1",
        Some("9P"),
        &["Tempel 1"],
        Comet,
        None,
        3.0,
        C_COMET,
        "9P"
    ));
    v.push({
        let mut e = sbdb!(
            "churyumov_gerasimenko",
            "67P/Churyumov-Gerasimenko",
            Some("67P"),
            &["Churyumov-Gerasimenko"],
            Comet,
            None,
            1.7,
            C_COMET,
            "67P"
        );
        e.source_note = "orbit: JPL SBDB heliocentric ECLIPJ2000; radius: SBDB diameter 3.4 +/- 0.1 km / 2 (Sierks et al. 2015)";
        e
    });
    v.push({
        let mut e = sbdb!(
            "hartley_2",
            "103P/Hartley 2",
            Some("103P"),
            &["Hartley 2"],
            Comet,
            None,
            0.8,
            C_COMET,
            "103P"
        );
        e.source_note = "orbit: JPL SBDB heliocentric ECLIPJ2000; radius: SBDB diameter 1.6 km / 2 (Lamy et al. 2004)";
        e
    });
    v.push(sbdb!(
        "hale_bopp",
        "Hale-Bopp",
        Some("C/1995 O1"),
        &[],
        Comet,
        None,
        30.0,
        C_COMET,
        "C/1995 O1"
    ));
    v.push(sbdb!(
        "neowise",
        "NEOWISE",
        Some("C/2020 F3"),
        &[],
        Comet,
        None,
        2.5,
        C_COMET,
        "C/2020 F3"
    ));
    v.push({
        let mut e = sbdb!(
            "3i_atlas",
            "3I/ATLAS",
            Some("C/2025 N1"),
            &["3I"],
            Comet,
            None,
            0.5,
            C_COMET,
            "C/2025 N1"
        );
        e.source_note = "orbit: JPL SBDB heliocentric ECLIPJ2000; nucleus radius adopted 0.5 km; HST constraint 0.16-2.8 km at pV=0.04 (NASA/HST 2025; arXiv:2512.22365); NGA-based estimates ~0.3 km";
        e
    });

    for entry in &mut v {
        entry.is_major_moon = MAJOR_MOON_IDS.contains(&entry.id);
    }
    v
}

/// Full source string for the emitted record.
pub fn source_string(e: &Entry) -> String {
    let description_source = curated_description(e.id).1;
    let visibility_note = (e.category == Category::Moon).then(|| {
        format!(
            "; display: WP10 major_moon={} curated under TASKS Q8 (2026-07-13)",
            e.is_major_moon
        )
    });
    format!(
        "{}; {}; description: {}{}",
        e.source_note,
        PHYS_NOTE,
        description_source,
        visibility_note.as_deref().unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn manifest_has_66_bodies_with_rev_b_category_counts() {
        let es = entries();
        assert_eq!(es.len(), 66);
        let count = |c: Category| es.iter().filter(|e| e.category == c).count();
        assert_eq!(count(Category::Star), 1);
        assert_eq!(count(Category::Planet), 8);
        assert_eq!(count(Category::DwarfPlanet), 9);
        assert_eq!(count(Category::Asteroid), 8);
        assert_eq!(count(Category::Moon), 32);
        assert_eq!(count(Category::Comet), 8);
    }

    #[test]
    fn texture_assignments_cover_the_star_planets_and_available_major_moons() {
        let entries = entries();
        for entry in entries
            .iter()
            .filter(|entry| matches!(entry.category, Category::Star | Category::Planet))
        {
            assert!(
                texture_path(entry.id).is_some(),
                "{} has no curated WP15 texture",
                entry.id
            );
        }
        let textured_moons: HashSet<_> = entries
            .iter()
            .filter(|entry| entry.category == Category::Moon)
            .filter(|entry| texture_path(entry.id).is_some())
            .map(|entry| entry.id)
            .collect();
        assert_eq!(
            textured_moons,
            HashSet::from(["moon", "io", "europa", "ganymede", "callisto", "titan"])
        );
        assert!(entries
            .iter()
            .filter(|entry| texture_path(entry.id).is_some())
            .all(|entry| entry.category != Category::Moon || entry.is_major_moon));
    }

    #[test]
    fn major_moon_membership_is_explicit_unique_and_covers_every_moon_system() {
        let entries = entries();
        let expected: HashSet<&str> = MAJOR_MOON_IDS.iter().copied().collect();
        assert_eq!(expected.len(), MAJOR_MOON_IDS.len(), "duplicate major id");

        let actual: HashSet<&str> = entries
            .iter()
            .filter(|entry| entry.is_major_moon)
            .map(|entry| entry.id)
            .collect();
        assert_eq!(actual, expected);
        assert!(entries
            .iter()
            .filter(|entry| entry.is_major_moon)
            .all(|entry| entry.category == Category::Moon));

        let moon_parents: HashSet<&str> = entries
            .iter()
            .filter(|entry| entry.category == Category::Moon)
            .filter_map(|entry| entry.parent)
            .collect();
        let major_parents: HashSet<&str> = entries
            .iter()
            .filter(|entry| entry.is_major_moon)
            .filter_map(|entry| entry.parent)
            .collect();
        assert_eq!(major_parents, moon_parents);
    }

    #[test]
    fn manifest_ids_unique_and_parents_precede_children() {
        let es = entries();
        let mut seen: HashSet<&str> = HashSet::new();
        for e in &es {
            assert!(seen.insert(e.id), "duplicate id {}", e.id);
            if let Some(p) = e.parent {
                assert!(
                    seen.contains(p),
                    "parent '{}' of '{}' must precede it",
                    p,
                    e.id
                );
            }
        }
    }

    #[test]
    fn every_body_has_two_to_four_reviewed_sentences_and_a_public_source() {
        for entry in entries() {
            let (description, description_source) = curated_description(entry.id);
            assert_eq!(
                entry.blurb, description,
                "description drift for {}",
                entry.id
            );
            assert!(
                !description.trim().is_empty(),
                "empty description for {}",
                entry.id
            );

            let sentence_count = description.matches(". ").count()
                + usize::from(description.trim_end().ends_with('.'));
            assert!(
                (2..=4).contains(&sentence_count),
                "{} has {sentence_count} sentences: {description}",
                entry.id
            );
            assert!(
                description_source.starts_with("https://"),
                "{} has a non-public description source: {description_source}",
                entry.id
            );
            assert!(
                description_source.contains("science.nasa.gov")
                    || description_source.contains("ssd.jpl.nasa.gov"),
                "{} has a non-authoritative description source: {description_source}",
                entry.id
            );
            assert!(
                source_string(&entry).contains(&format!("description: {description_source}")),
                "{} does not emit its description provenance",
                entry.id
            );
        }
    }

    #[test]
    fn every_parent_has_gm() {
        let es = entries();
        let parents: HashSet<&str> = es.iter().filter_map(|e| e.parent).collect();
        for e in &es {
            if parents.contains(e.id) {
                assert!(e.gm_km3_s2.is_some(), "parent '{}' missing GM", e.id);
            }
        }
    }

    #[test]
    fn planet_routes_split_inner_centers_from_outer_barycenters() {
        let es = entries();
        let expected = [
            ("mercury", "199"),
            ("venus", "299"),
            ("earth", "399"),
            ("mars", "499"),
            ("jupiter", "5"),
            ("saturn", "6"),
            ("uranus", "7"),
            ("neptune", "8"),
        ];

        for (id, expected_command) in expected {
            let entry = es
                .iter()
                .find(|entry| entry.id == id)
                .unwrap_or_else(|| panic!("missing planet '{id}'"));
            assert_eq!(
                entry.route,
                Route::HorizonsPlanet {
                    command: expected_command,
                },
                "unexpected Horizons route for '{id}'"
            );
        }
    }

    #[test]
    fn approved_curated_values_carry_provenance() {
        let es = entries();
        let pluto = es.iter().find(|entry| entry.id == "pluto").unwrap();

        assert_eq!(pluto.gm_km3_s2, Some(869.6 + 105.9));
        assert!(source_string(pluto).contains("Pluto+Charon system 975.5 km^3/s^2"));

        let expected = [
            ("himalia", 85.0, "85 +/- 10 km"),
            ("nix", 18.0, "Stern et al. 2018"),
            ("hydra", 18.5, "Stern et al. 2018"),
            ("hiiaka", 185.0, "Fernandez-Valenzuela et al. 2025"),
            ("namaka", 75.0, "Muller et al. 2019"),
            ("hygiea", 203.56, "SBDB diameter 407.12"),
            ("churyumov_gerasimenko", 1.7, "Sierks et al. 2015"),
            ("hartley_2", 0.8, "Lamy et al. 2004"),
            ("3i_atlas", 0.5, "HST constraint 0.16-2.8 km"),
        ];
        for (id, radius_km, provenance) in expected {
            let entry = es
                .iter()
                .find(|entry| entry.id == id)
                .unwrap_or_else(|| panic!("missing curated body '{id}'"));
            assert_eq!(entry.radius_km, radius_km, "unexpected radius for '{id}'");
            let source = source_string(entry);
            assert!(
                source.contains(provenance),
                "missing physical provenance for '{id}': {source}"
            );
            assert!(source.contains("curated radius reviewed 2026-07-13"));
        }
    }

    #[test]
    fn approved_de440_sun_and_planet_gms_carry_provenance() {
        let es = entries();
        let expected = [
            ("sun", 132_712_440_041.279_42),
            ("mercury", 22_031.868_551),
            ("venus", 324_858.592_000),
            ("earth", 398_600.435_507),
            ("mars", 42_828.375_816),
            ("jupiter", 126_712_764.100_000),
            ("saturn", 37_940_584.841_800),
            ("uranus", 5_794_556.400_000),
            ("neptune", 6_836_527.100_580),
        ];

        for (id, expected_gm) in expected {
            let entry = es
                .iter()
                .find(|entry| entry.id == id)
                .unwrap_or_else(|| panic!("missing DE440 body '{id}'"));
            assert_eq!(entry.gm_km3_s2, Some(expected_gm));
            assert!(
                source_string(entry).contains("GM: JPL DE440 (Park et al. 2021)"),
                "missing DE440 GM provenance for '{id}'"
            );
        }
    }
}
