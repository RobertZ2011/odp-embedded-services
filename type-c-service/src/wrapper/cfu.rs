//! CFU message bridge
//! TODO: remove this once we have a more generic FW update implementation
use embedded_cfu_protocol::protocol_definitions::*;
use embedded_services::cfu::component::{RequestData, InternalResponseData};
use embedded_services::fw_update::{Error as FwError, FwUpdate as FwUpdateTrait};
use embedded_services::type_c::controller::Controller;
use embedded_services::{debug, error};

use super::*;

impl<const N: usize, C: Controller + FwUpdateTrait> ControllerWrapper<'_, N, C> {
    /// Process a GetFwVersion command
    async fn process_get_fw_version(&self, target: &mut C) -> InternalResponseData {
        let version = match target.get_active_fw_version().await {
            Ok(v) => v,
            Err(FwError::General(e)) => {
                error!("Failed to get active firmware version: {:?}", e);
                return InternalResponseData::ComponentBusy;
            }
            Err(FwError::Bus(_)) => {
                error!("Failed to get active firmware version, bus error");
                return InternalResponseData::ComponentBusy;
            }
        };

        let dev_inf = FwVerComponentInfo::new(FwVersion::new(version), self.cfu_device.component_id());
        let comp_info: [FwVerComponentInfo; MAX_CMPT_COUNT] = [dev_inf; MAX_CMPT_COUNT];
        InternalResponseData::FwVersionResponse(GetFwVersionResponse {
            header: GetFwVersionResponseHeader::new(1, GetFwVerRespHeaderByte3::NoSpecialFlags),
            component_info: comp_info,
        })
    }

    /// Process a GiveOffer command
    async fn process_give_offer(&self, offer: &FwUpdateOffer) -> Option<InternalResponseData> {
        // accept any and all offers regardless of what version it is
        if offer.component_info.component_id == self.cfu_device.component_id() {
            debug!("Accepting offer");
            Some(InternalResponseData::OfferResponse(FwUpdateOfferResponse::new_accept(
                HostToken::Driver,
            )))
        } else {
            debug!("Rejecting offer, ID mimismatch");
            None
        }
    }

    /// Process a GiveContent command
    async fn process_give_content(
        &self,
        controller: &mut C,
        state: &mut InternalState,
        content: &FwUpdateContentCommand,
    ) -> Option<InternalResponseData> {
        let data = &content.data[0..content.header.data_length as usize];
        if content.header.flags & FW_UPDATE_FLAG_FIRST_BLOCK != 0 {
            debug!("Got first block");

            // Need to start the update
            if controller.start_fw_update().await.is_err() {
                error!("Failed to start FW update");
                return Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(
                    content.header.sequence_num,
                    CfuUpdateContentResponseStatus::ErrorPrepare,
                )));
            }

            state.fw_update = true;
        }

        if controller
            .write_fw_contents(content.header.firmware_address as usize, data)
            .await
            .is_err()
        {
            error!("Failed to write block");
            return Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(
                content.header.sequence_num,
                CfuUpdateContentResponseStatus::ErrorWrite,
            )));
        }

        Some(InternalResponseData::ContentResponse(FwUpdateContentResponse::new(
            content.header.sequence_num,
            CfuUpdateContentResponseStatus::Success,
        )))
    }

    /// Process a FinalizeUpdate command
    async fn process_finalize_update(
        &self,
        controller: &mut C,
        state: &mut InternalState,
    ) -> Option<InternalResponseData> {
        if controller.finalize_fw_update().await.is_err() {
            error!("Failed to finalize FW update");
        }
        state.fw_update = false;
        None
    }

    /// Process a CFU command
    pub async fn process_cfu_command(
        &self,
        controller: &mut C,
        state: &mut InternalState,
        command: &RequestData,
    ) -> Option<InternalResponseData> {
        match command {
            RequestData::FwVersionRequest => {
                debug!("Got FwVersionRequest");
                Some(self.process_get_fw_version(controller).await)
            }
            RequestData::GiveOffer(offer) => {
                debug!("Got GiveOffer");
                self.process_give_offer(offer).await
            }
            RequestData::GiveContent(content) => {
                debug!("Got GiveContent");
                self.process_give_content(controller, state, content).await
            }
            RequestData::FinalizeUpdate => {
                debug!("Got FinalizeUpdate");
                self.process_finalize_update(controller, state).await
            }
            _ => {
                debug!("Got other command: {:#?}", command);
                None
            }
        }
    }

    /// Sends a CFU response to the command
    pub async fn send_cfu_response(&self, response: Option<InternalResponseData>) {
        self.cfu_device
            .send_response(response.unwrap_or(InternalResponseData::ComponentPrepared))
            .await;
    }
}
