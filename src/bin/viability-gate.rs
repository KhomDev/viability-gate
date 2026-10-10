//! The `viability-gate` binary.
//!
//! Identical to `vg`. This name exists because `vg` collides with an unrelated
//! bioinformatics tool.

fn main() -> std::process::ExitCode {
    viability_gate::cli::main_entry()
}
