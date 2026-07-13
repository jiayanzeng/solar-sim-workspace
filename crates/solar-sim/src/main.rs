//! Desktop entry point; the WP4 application and testable systems live in the
//! library so propagation and floating-origin behavior can be verified headlessly.

fn main() {
    solar_sim::run_from_env();
}
