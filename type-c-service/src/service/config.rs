use embedded_usb_pd::ucsi;

/// UCSI battery charging capability status configuration
///
/// The implementation checks these thresholds in order from not_battery_charging_mw to slow_battery_charging_mw.
/// Misordering the thresholds will lead to the first matching threshold being selected.
#[derive(Debug, Clone, Copy, Default)]
pub struct UcsiBatteryChargingThresholdConfig {
    /// No battery charging power threshold in milliwatts
    ///
    /// Below this level GET_CONNECTOR_STATUS will report no battery charging
    pub not_battery_charging_mw: Option<u32>,
    /// Very slow battery charging power threshold in milliwatts
    ///
    /// Below this level GET_CONNECTOR_STATUS will report very slow battery charging
    pub very_slow_battery_charging_mw: Option<u32>,
    /// Slow battery charging power threshold in milliwatts
    ///
    /// Below this level GET_CONNECTOR_STATUS will report slow battery charging
    pub slow_battery_charging_mw: Option<u32>,
}

impl UcsiBatteryChargingThresholdConfig {
    /// Try to create a new Self with validation
    pub fn try_new(
        not_battery_charging_mw: Option<u32>,
        very_slow_battery_charging_mw: Option<u32>,
        slow_battery_charging_mw: Option<u32>,
    ) -> Option<Self> {
        if not_battery_charging_mw
            .zip(very_slow_battery_charging_mw)
            .is_some_and(|(not, very_slow)| not >= very_slow)
        {
            return None;
        }

        if very_slow_battery_charging_mw
            .zip(slow_battery_charging_mw)
            .is_some_and(|(very_slow, slow)| very_slow >= slow)
        {
            return None;
        }

        if not_battery_charging_mw
            .zip(slow_battery_charging_mw)
            .is_some_and(|(not, slow)| not >= slow)
        {
            return None;
        }

        Some(Self {
            not_battery_charging_mw,
            very_slow_battery_charging_mw,
            slow_battery_charging_mw,
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ucsi_battery_charging_threshold_config_ordering() {
        // Valid ordering
        assert!(UcsiBatteryChargingThresholdConfig::try_new(Some(1000), Some(2000), Some(3000)).is_some());
        assert!(UcsiBatteryChargingThresholdConfig::try_new(None, Some(2000), Some(3000)).is_some());
        assert!(UcsiBatteryChargingThresholdConfig::try_new(Some(1000), None, Some(3000)).is_some());
        assert!(UcsiBatteryChargingThresholdConfig::try_new(Some(1000), Some(2000), None).is_some());
        assert!(UcsiBatteryChargingThresholdConfig::try_new(None, None, None).is_some());

        // Invalid ordering
        assert!(UcsiBatteryChargingThresholdConfig::try_new(Some(3000), Some(2000), Some(1000)).is_none());
        assert!(UcsiBatteryChargingThresholdConfig::try_new(Some(2000), Some(2000), Some(3000)).is_none());
        assert!(UcsiBatteryChargingThresholdConfig::try_new(Some(1000), Some(1000), Some(3000)).is_none());
        assert!(UcsiBatteryChargingThresholdConfig::try_new(Some(1000), Some(2000), Some(2000)).is_none());
        assert!(UcsiBatteryChargingThresholdConfig::try_new(Some(3000), None, Some(1000)).is_none());
    }
}
