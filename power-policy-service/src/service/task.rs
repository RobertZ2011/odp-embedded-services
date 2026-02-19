use embedded_services::{error, info, sync::Lockable};

use crate::psu::Psu;

use super::Service;

/// Runs the power policy task.
pub async fn task<'a, const PSU_COUNT: usize, S: Lockable<Inner = Service<'a, PSU>>, PSU: Lockable>(
    mut psu_events: crate::psu::event::EventReceivers<'a, PSU_COUNT, PSU>,
    policy: &'a S,
) -> !
where
    PSU::Inner: Psu,
{
    info!("Starting power policy task");
    loop {
        let event = psu_events.wait_event().await;

        if let Err(e) = policy.lock().await.process_psu_event(event).await {
            error!("Error processing request: {:?}", e);
        }
    }
}
