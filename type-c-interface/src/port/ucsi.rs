//! Traits and types related to UCSI operation
use embedded_usb_pd::{PdError, ucsi::lpm};

/// Trait for ports that support UCSI operations
///
/// This isn't a super type of [`super::Pd`] because it's possible to implement UCSI functionality
/// without directly exposing lower-level PD functionality.
pub trait Ucsi {
    /// Execute the given UCSI command
    fn execute_ucsi_command(
        &mut self,
        command: lpm::LocalCommand,
    ) -> impl Future<Output = Result<Option<lpm::ResponseData>, PdError>>;
}
