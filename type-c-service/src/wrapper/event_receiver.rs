//! This module contains event receiver types for the controller wrapper.
use core::array;
use core::future::pending;
use core::pin::pin;
use embassy_futures::select::{Either, select, select_slice};
use embassy_time::{Instant, Timer};
use embedded_services::debug;
use embedded_usb_pd::LocalPortId;

use crate::PortEventStreamer;
use crate::wrapper::message::{Event, LocalPortEvent};
use type_c_interface::port::event::{PortEvent, PortEventBitfield, PortStatusEventBitfield};

/// Trait used for receiving interrupt from the controller.
pub trait InterruptReceiver<const N: usize> {
    /// Wait for the next interrupt event.
    fn wait_interrupt(&mut self) -> impl Future<Output = [PortEventBitfield; N]>;
}

/// Struct to receive and stream port events from the controller.
pub struct PortEventReceiver<const N: usize, Receiver: InterruptReceiver<N>> {
    /// Receiver for the controller's interrupt events
    receiver: Receiver,
    /// Port event streaming state
    streaming_state: Option<PortEventStreamer<array::IntoIter<PortEventBitfield, N>>>,
}

impl<const N: usize, Receiver: InterruptReceiver<N>> PortEventReceiver<N, Receiver> {
    /// Create a new instance
    pub fn new(receiver: Receiver) -> Self {
        Self {
            receiver,
            streaming_state: None,
        }
    }

    /// Wait for the next port event
    pub async fn wait_next(&mut self) -> LocalPortEvent {
        loop {
            let streaming_state = if let Some(streaming_state) = &mut self.streaming_state {
                // Yield to ensure we don't monopolize the executor
                embassy_futures::yield_now().await;
                streaming_state
            } else {
                let events = self.receiver.wait_interrupt().await;
                self.streaming_state.insert(PortEventStreamer::new(events.into_iter()))
            };

            if let Some((port_index, event)) = streaming_state.next() {
                return LocalPortEvent {
                    port: LocalPortId(port_index as u8),
                    event,
                };
            } else {
                self.streaming_state = None;
            }
        }
    }
}

/// Struct to receive sink ready timeout events.
pub struct SinkReadyTimeoutEvent<const N: usize> {
    timeouts: [Option<Instant>; N],
}

impl<const N: usize> SinkReadyTimeoutEvent<N> {
    /// Create a new instance
    pub fn new() -> Self {
        Self { timeouts: [None; N] }
    }

    /// Set a timeout for a specific port
    pub fn set_timeout(&mut self, port: LocalPortId, new_timeout: Instant) {
        let index = port.0 as usize;
        if let Some(timeout) = self.timeouts.get_mut(index) {
            *timeout = Some(new_timeout);
        }
    }

    /// Clear the timeout for a specific port
    pub fn clear_timeout(&mut self, port: LocalPortId) {
        let index = port.0 as usize;
        if let Some(timeout) = self.timeouts.get_mut(index) {
            *timeout = None;
        }
    }

    pub fn get_timeout(&self, port: LocalPortId) -> Option<Instant> {
        let index = port.0 as usize;
        self.timeouts.get(index).copied().flatten()
    }

    /// Wait for a sink ready timeout and return the port that has timed out.
    ///
    /// DROP SAFETY: No state to restore
    pub async fn wait_next(&mut self) -> LocalPortId {
        let mut futures = heapless::Vec::<_, N>::new();
        for (i, timeout) in self.timeouts.iter().enumerate() {
            let timeout = *timeout;
            // Size is fixed at compile time, so no chance of overflow
            let _ = futures.push(async move {
                if let Some(timeout) = timeout {
                    Timer::at(timeout).await;
                    debug!("Port{}: Sink ready timeout reached", i);
                } else {
                    pending::<()>().await;
                }
            });
        }

        // DROP SAFETY: Select over drop safe futures
        let (_, port_index) = select_slice(pin!(futures.as_mut_slice())).await;
        if let Some(timeout) = self.timeouts.get_mut(port_index) {
            *timeout = None;
        }
        LocalPortId(port_index as u8)
    }
}

impl<const N: usize> Default for SinkReadyTimeoutEvent<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Struct used for containing controller event receivers.
pub struct ArrayPortEventReceivers<const N: usize, PortInterrupts: InterruptReceiver<N>> {
    /// Port event receiver
    pub port_events: PortEventReceiver<N, PortInterrupts>,
    /// Sink ready timeout event receiver
    pub sink_ready_timeout: SinkReadyTimeoutEvent<N>,
}

impl<const N: usize, PortInterrupts: InterruptReceiver<N>> ArrayPortEventReceivers<N, PortInterrupts> {
    /// Create a new instance
    pub fn new(port_interrupts: PortInterrupts) -> Self {
        Self {
            port_events: PortEventReceiver::new(port_interrupts),
            sink_ready_timeout: SinkReadyTimeoutEvent::new(),
        }
    }

    /// Wait for the next port event from any port.
    ///
    /// Returns the local port ID and the event bitfield.
    pub async fn wait_event(&mut self) -> Event {
        match select(self.port_events.wait_next(), self.sink_ready_timeout.wait_next()).await {
            Either::First(event) => Event::PortEvent(event),
            Either::Second(port) => {
                let mut status_event = PortStatusEventBitfield::none();
                status_event.set_sink_ready(true);
                Event::PortEvent(LocalPortEvent {
                    port,
                    event: PortEvent::StatusChanged(status_event),
                })
            }
        }
    }
}
