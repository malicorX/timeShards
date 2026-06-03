use std::sync::OnceLock;

static EFFECTIVE_ADAPTER: OnceLock<&'static str> = OnceLock::new();

/// Value from `TIMESHARDS_HW_ADAPTER` (`sim`, `external`, or `unknown` for typos).
pub fn hardware_adapter_configured() -> &'static str {
    match std::env::var("TIMESHARDS_HW_ADAPTER")
        .unwrap_or_else(|_| "sim".into())
        .to_ascii_lowercase()
        .as_str()
    {
        "sim" | "simulator" => "sim",
        "external" | "primion" => "external",
        _ => "unknown",
    }
}

/// Active adapter after `bootstrap_hardware` (falls back to `sim` when configured value is unknown).
pub fn hardware_adapter_active() -> &'static str {
    *EFFECTIVE_ADAPTER.get().unwrap_or(&"sim")
}

pub fn set_effective_hardware_adapter(id: &'static str) {
    let _ = EFFECTIVE_ADAPTER.set(id);
}

/// Back-compat alias for configured env value (prefer `hardware_adapter_active` in health/runtime).
pub fn hardware_adapter_id() -> &'static str {
    hardware_adapter_configured()
}

pub fn is_simulator_adapter() -> bool {
    matches!(hardware_adapter_active(), "sim")
}

/// When set (e.g. `127.0.0.1:47831`), external adapter accepts newline-delimited JSON credential lines.
pub fn hardware_tcp_listen_addr() -> Option<String> {
    std::env::var("TIMESHARDS_HW_TCP_ADDR")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
