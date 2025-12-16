use embedded_usb_pd::ucsi;

#[derive(Debug, Clone, Copy, Default)]
pub struct UcsiBatteryChargingThresholdConfig {
    /// No battery charging power threshold in milliwatts
    ///
    /// Below this level GET_CONNECTOR_STATUS will report no battery charging
    pub no_battery_charging_mw: Option<u32>,
    /// Very slow battery charging power threshold in milliwatts
    ///
    /// Below this level GET_CONNECTOR_STATUS will report very slow battery charging
    pub very_slow_battery_charging_mw: Option<u32>,
    /// Slow battery charging power threshold in milliwatts
    ///
    /// Below this level GET_CONNECTOR_STATUS will report slow battery charging
    pub slow_battery_charging_mw: Option<u32>,
}

/// Type-c service configuration
#[derive(Debug, Clone, Copy, Default)]
pub struct Config {
    /// UCSI capabilities
    pub ucsi_capabilities: ucsi::ppm::get_capability::ResponseData,
    /// Optional override for UCSI port capabilities
    pub ucsi_port_capabilities: Option<ucsi::lpm::get_connector_capability::ResponseData>,
    /// UCSI battery charging configuration
    pub ucsi_battery_charging_config: UcsiBatteryChargingThresholdConfig,
}
